// Vantage setup: a themed WPF front end over the product's MSI, which stays the owner of the files,
// shortcuts and upgrades. The MSI is embedded as a resource and driven through the Windows Installer
// API, so the window shows real progress. Silent switches match msiexec's: /quiet or /passive,
// /norestart, /log <file>, /uninstall, INSTALLFOLDER=<dir>.
//
// Elevation: the window runs as the user; pressing Install relaunches this exe elevated with --go, which
// opens straight on the progress page. `--render <dir>` writes a picture of each page and exits, for
// checking the design without showing a window.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text;
using System.Threading;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Media.Imaging;

static class Msi {
    public delegate int UiHandler(IntPtr context, uint type, [MarshalAs(UnmanagedType.LPWStr)] string message);
    [DllImport("msi.dll", CharSet = CharSet.Unicode)] public static extern int MsiSetInternalUI(int level, IntPtr window);
    [DllImport("msi.dll", CharSet = CharSet.Unicode)] public static extern UiHandler MsiSetExternalUI(UiHandler handler, uint filter, IntPtr context);
    [DllImport("msi.dll", CharSet = CharSet.Unicode)] public static extern int MsiInstallProduct(string package, string commandLine);
    [DllImport("msi.dll", CharSet = CharSet.Unicode)] public static extern int MsiConfigureProductEx(string product, int level, int state, string commandLine);
    [DllImport("msi.dll", CharSet = CharSet.Unicode)] public static extern int MsiEnumRelatedProducts(string upgradeCode, int reserved, int index, StringBuilder product);
    [DllImport("msi.dll", CharSet = CharSet.Unicode)] public static extern int MsiGetProductInfo(string product, string property, StringBuilder value, ref int length);

    public const uint FilterProgress = 0x400, FilterAction = 0x100, FilterError = 0x3;

    /// The product code of the installed Vantage, or null.
    public static string Installed(string upgradeCode) {
        var product = new StringBuilder(39);
        return MsiEnumRelatedProducts(upgradeCode, 0, 0, product) == 0 ? product.ToString() : null;
    }
    public static string Info(string product, string property) {
        int length = 1024; var value = new StringBuilder(length);
        return MsiGetProductInfo(product, property, value, ref length) == 0 ? value.ToString() : null;
    }
}

/// Turns Windows Installer's progress messages into one steady 0..1 value and a readable status.
sealed class Progress {
    const int Phases = 3;
    long total, done; int resets; bool forward = true, counting;
    public double Fraction;
    public string Status = "Preparing...";
    public bool Cancel;

    static readonly Dictionary<string, string> Steps = new Dictionary<string, string> {
        {"InstallValidate", "Checking your system"}, {"RemoveExistingProducts", "Removing the previous version"},
        {"RemoveFiles", "Removing files"}, {"InstallFiles", "Copying files"}, {"CreateShortcuts", "Creating shortcuts"},
        {"RemoveShortcuts", "Removing shortcuts"}, {"WriteRegistryValues", "Registering Vantage"},
        {"RemoveRegistryValues", "Cleaning up"}, {"PublishProduct", "Finishing up"}, {"InstallFinalize", "Finishing up"},
    };

    static int Field(string message, int index) {
        int at = message.IndexOf(index + ": ");
        if (at < 0) return 0;
        at += 3; int end = at;
        while (end < message.Length && (char.IsDigit(message[end]) || message[end] == '-')) end++;
        int value; return int.TryParse(message.Substring(at, end - at), out value) ? value : 0;
    }

    public int Handle(IntPtr context, uint type, string message) {
        uint kind = type & 0xFF000000;
        if (kind == 0x0A000000) { // progress
            switch (Field(message, 1)) {
                case 0: total = Field(message, 2); forward = Field(message, 3) == 0; counting = Field(message, 4) == 1; done = 0; resets++; break;
                case 1: counting = Field(message, 3) == 1 || counting; break;
                case 2: if (total > 0) done += forward ? Field(message, 2) : -Field(message, 2); break;
                case 3: total += Field(message, 2); break;
            }
            if (total > 0) {
                double phase = Math.Max(0.0, Math.Min(1.0, (double)done / total));
                double all = (Math.Max(1, resets) - 1 + phase) / Phases;
                if (all > Fraction) Fraction = Math.Min(1.0, all);
            }
        } else if (kind == 0x08000000) { // action start: "Action start 12:00:00: Name."
            int colon = message.LastIndexOf(": ");
            if (colon >= 0) {
                string name = message.Substring(colon + 2).Trim().TrimEnd('.');
                string friendly; if (Steps.TryGetValue(name, out friendly)) Status = friendly;
            }
        }
        return Cancel ? 2 : 1;
    }
}

static class Program {
    const string UpgradeCode = "{A3036D81-6368-4CA1-964B-CC43A2335337}";
    static readonly string[] Pages = { "Welcome", "Maintain", "Progress", "Done", "Error" };

    static FrameworkElement root;
    static Window window;
    static string folder, product, version, logPath;
    static Action primary, secondary;

    static T Find<T>(string name) where T : class { return root.FindName(name) as T; }
    static Stream Resource(string name) { return Assembly.GetExecutingAssembly().GetManifestResourceStream(name); }
    static bool IsAdmin() { return new WindowsPrincipal(WindowsIdentity.GetCurrent()).IsInRole(WindowsBuiltInRole.Administrator); }

    static string Arg(string[] args, string name) {
        for (int i = 0; i + 1 < args.Length; i++) if (args[i].Equals(name, StringComparison.OrdinalIgnoreCase)) return args[i + 1];
        return null;
    }
    static bool Has(string[] args, params string[] names) {
        foreach (var a in args) foreach (var n in names) if (a.Equals(n, StringComparison.OrdinalIgnoreCase)) return true;
        return false;
    }

    static string DefaultFolder() {
        return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), "Vantage");
    }

    static BitmapImage Mark() {
        var image = new BitmapImage();
        image.BeginInit(); image.StreamSource = Resource("mark.png"); image.CacheOption = BitmapCacheOption.OnLoad; image.EndInit(); image.Freeze();
        return image;
    }

    static void Build() {
        using (var xaml = Resource("Setup.xaml")) root = (FrameworkElement)XamlReader.Load(xaml);
        ((Border)root).SetValue(System.Windows.Documents.TextElement.FontFamilyProperty, new FontFamily("Segoe UI Variable Display, Segoe UI"));
        TextOptions.SetTextFormattingMode(root, TextFormattingMode.Ideal);
        Find<Image>("Mark").Source = Mark();
        RenderOptions.SetBitmapScalingMode(Find<Image>("Mark"), BitmapScalingMode.HighQuality);
        using (var v = new StreamReader(Resource("version.txt"))) version = v.ReadToEnd().Trim();
        Find<TextBlock>("VersionText").Text = "Version " + version;
        Find<Button>("PrimaryBtn").Click += delegate { if (primary != null) primary(); };
        Find<Button>("SecondaryBtn").Click += delegate { if (secondary != null) secondary(); };
    }

    static void Show(string page, string primaryText, Action onPrimary, string secondaryText, Action onSecondary) {
        foreach (var name in Pages) {
            var element = Find<UIElement>("Page" + name);
            bool on = name == page;
            element.Visibility = on ? Visibility.Visible : Visibility.Collapsed;
            if (on && window != null) element.BeginAnimation(UIElement.OpacityProperty, new DoubleAnimation(0, 1, TimeSpan.FromMilliseconds(260)));
        }
        var p = Find<Button>("PrimaryBtn"); var s = Find<Button>("SecondaryBtn");
        p.Content = primaryText; p.Visibility = primaryText == null ? Visibility.Collapsed : Visibility.Visible;
        s.Content = secondaryText; s.Visibility = secondaryText == null ? Visibility.Collapsed : Visibility.Visible;
        primary = onPrimary; secondary = onSecondary;
    }

    static void ShowWelcome() {
        Find<TextBlock>("FolderText").Text = folder;
        Find<Button>("ChangeFolder").Visibility = Visibility.Visible;
        Show("Welcome", "Install", delegate { Begin("install"); }, "Cancel", Quit);
    }
    static void ShowMaintain() {
        Show("Maintain", "Repair", delegate { Begin("repair"); }, "Uninstall", delegate { Begin("uninstall"); });
    }
    static void ShowDone(string title, string text, bool launch) {
        Find<TextBlock>("DoneTitle").Text = title; Find<TextBlock>("DoneText").Text = text;
        if (launch) Show("Done", "Launch Vantage", delegate { Launch(); Quit(); }, "Close", Quit);
        else Show("Done", "Close", Quit, null, null);
    }
    static void ShowError(string text) {
        Find<TextBlock>("ErrorText").Text = text;
        Show("Error", "Close", Quit, null, null);
    }
    static void ShowProgress(string title) {
        Find<TextBlock>("ProgressTitle").Text = title;
        Show("Progress", null, null, "Cancel", delegate { Cancel.Cancel = true; Find<TextBlock>("ProgressStatus").Text = "Cancelling..."; });
    }

    static void Quit() { if (window != null) window.Close(); }
    static void Launch() {
        try { Process.Start(Path.Combine(folder, "cinnabar.exe")); } catch { }
    }

    static Progress Cancel = new Progress();

    /// Starts `action`, relaunching elevated first when needed.
    static void Begin(string action) {
        if (!IsAdmin()) {
            var info = new ProcessStartInfo(Assembly.GetExecutingAssembly().Location,
                "--go " + action + " --folder \"" + folder + "\"") { Verb = "runas", UseShellExecute = true };
            try { Process.Start(info); Quit(); } catch { /* the user declined the prompt */ }
            return;
        }
        string title = action == "install" ? "Installing Vantage" : action == "repair" ? "Repairing Vantage" : "Removing Vantage";
        ShowProgress(title);
        var bar = Find<Border>("BarFill"); var track = (FrameworkElement)bar.Parent is Grid ? (FrameworkElement)bar.Parent : bar;
        var status = Find<TextBlock>("ProgressStatus"); var percent = Find<TextBlock>("ProgressPercent");
        int result = -1;
        var worker = new Thread(delegate () { result = Run(action, Cancel); });
        worker.IsBackground = true; worker.Start();
        var timer = new System.Windows.Threading.DispatcherTimer { Interval = TimeSpan.FromMilliseconds(60) };
        double shown = 0;
        timer.Tick += delegate {
            shown += (Cancel.Fraction - shown) * 0.25;
            bar.Width = Math.Max(0, track.ActualWidth * shown);
            percent.Text = (int)(shown * 100) + "%";
            if (!Cancel.Cancel) status.Text = Cancel.Status;
            if (worker.IsAlive) return;
            timer.Stop();
            Finish(action, result);
        };
        timer.Start();
    }

    static void Finish(string action, int code) {
        if (code == 0 || code == 3010) {
            if (action == "uninstall") ShowDone("Vantage was removed", "Thanks for trying it. You can install it again at any time.", false);
            else ShowDone(action == "repair" ? "Repair complete" : "You are all set",
                "Vantage is installed. Launch it now, or later from the Start menu or your desktop.", true);
        } else if (code == 1602) {
            ShowError("Setup was cancelled. Nothing was changed.");
        } else {
            ShowError("Windows Installer reported error " + code + ". Close other installers and try again; the log, if requested with /log, has the details.");
        }
    }

    /// Runs the MSI action on the calling thread and returns Windows Installer's result.
    static int Run(string action, Progress progress) {
        var temp = Path.Combine(Path.GetTempPath(), "vantage-setup-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(temp);
        var msi = Path.Combine(temp, "Vantage.msi");
        try {
            using (var input = Resource("Vantage.msi")) using (var output = File.Create(msi)) input.CopyTo(output);
            Msi.UiHandler handler = delegate (IntPtr c, uint t, string m) { Log(m); return progress.Handle(c, t, m); };
            Msi.MsiSetInternalUI(2, IntPtr.Zero);
            Msi.MsiSetExternalUI(handler, Msi.FilterProgress | Msi.FilterAction | Msi.FilterError, IntPtr.Zero);
            string reboot = "REBOOT=ReallySuppress ";
            int code;
            if (action == "uninstall") code = Msi.MsiConfigureProductEx(product, 0, 2, reboot);
            else if (action == "repair") code = Msi.MsiInstallProduct(msi, reboot + "REINSTALL=ALL REINSTALLMODE=vomus");
            else code = Msi.MsiInstallProduct(msi, reboot + "INSTALLFOLDER=\"" + folder + "\"");
            Log("result " + code);
            progress.Fraction = 1;
            GC.KeepAlive(handler);
            return code;
        } catch (Exception e) { Log(e.ToString()); return 1603; }
        finally { try { Directory.Delete(temp, true); } catch { } }
    }

    static void Log(string line) {
        if (logPath == null || line == null) return;
        try { lock (typeof(Program)) File.AppendAllText(logPath, line + Environment.NewLine); } catch { }
    }

    static void ChooseFolder() {
        using (var dialog = new System.Windows.Forms.FolderBrowserDialog { Description = "Choose where to install Vantage", SelectedPath = folder, ShowNewFolderButton = true }) {
            if (dialog.ShowDialog() != System.Windows.Forms.DialogResult.OK) return;
            folder = dialog.SelectedPath;
            if (!Path.GetFileName(folder.TrimEnd('\\')).Equals("Vantage", StringComparison.OrdinalIgnoreCase)) folder = Path.Combine(folder, "Vantage");
            Find<TextBlock>("FolderText").Text = folder;
        }
    }

    /// Writes one picture per page, for reviewing the design without showing a window.
    static void RenderPages(string dir) {
        Directory.CreateDirectory(dir);
        root.Measure(new Size(820, 520)); root.Arrange(new Rect(0, 0, 820, 520));
        Action<string> save = delegate (string name) {
            root.UpdateLayout();
            var bmp = new RenderTargetBitmap(820, 520, 96, 96, PixelFormats.Pbgra32); bmp.Render(root);
            var enc = new PngBitmapEncoder(); enc.Frames.Add(BitmapFrame.Create(bmp));
            using (var f = File.Create(Path.Combine(dir, name + ".png"))) enc.Save(f);
        };
        ShowWelcome(); save("welcome");
        ShowMaintain(); save("maintain");
        ShowProgress("Installing Vantage");
        Find<TextBlock>("ProgressStatus").Text = "Copying files"; Find<TextBlock>("ProgressPercent").Text = "62%";
        root.UpdateLayout();
        var bar = Find<Border>("BarFill"); bar.Width = ((FrameworkElement)bar.Parent).ActualWidth * 0.62; save("progress");
        ShowDone("You are all set", "Vantage is installed. Launch it now, or later from the Start menu or your desktop.", true); save("done");
        ShowError("Windows Installer reported error 1603. Close other installers and try again."); save("error");
    }

    [STAThread]
    static int Main(string[] args) {
        folder = Arg(args, "--folder") ?? DefaultFolder();
        foreach (var a in args) if (a.StartsWith("INSTALLFOLDER=", StringComparison.OrdinalIgnoreCase)) folder = a.Substring(14).Trim('"');
        logPath = Arg(args, "/log") ?? Arg(args, "-log");
        product = Msi.Installed(UpgradeCode);
        if (product != null && Arg(args, "--folder") == null) {
            var location = Msi.Info(product, "InstallLocation");
            if (!string.IsNullOrEmpty(location)) folder = location.TrimEnd('\\');
        }

        bool quiet = Has(args, "/quiet", "/q", "/qn", "/passive", "/silent", "-quiet", "-silent");
        string action = Has(args, "/uninstall", "-uninstall") ? "uninstall" : Arg(args, "--go");
        if (quiet) {
            string mode = action ?? "install";
            if (!IsAdmin()) { // one elevated copy does the work, with the same switches
                var info = new ProcessStartInfo(Assembly.GetExecutingAssembly().Location, Environment.CommandLine.Substring(Environment.CommandLine.IndexOf(".exe", StringComparison.OrdinalIgnoreCase) + 5).TrimStart('"', ' '))
                    { Verb = "runas", UseShellExecute = true };
                try { var child = Process.Start(info); child.WaitForExit(); return child.ExitCode; } catch { return 1602; }
            }
            return Run(mode, new Progress());
        }

        var app = new Application { ShutdownMode = ShutdownMode.OnMainWindowClose };
        Build();
        string extract = Arg(args, "--extract");
        if (extract != null) { // the embedded MSI, for the release checks
            Directory.CreateDirectory(extract);
            using (var input = Resource("Vantage.msi")) using (var output = File.Create(Path.Combine(extract, "Vantage.msi"))) input.CopyTo(output);
            return 0;
        }
        string render = Arg(args, "--render");
        if (render != null) { RenderPages(render); return 0; }

        window = new Window {
            Content = root, Width = 820, Height = 520, WindowStyle = WindowStyle.None, AllowsTransparency = true,
            Background = Brushes.Transparent, ResizeMode = ResizeMode.CanMinimize, WindowStartupLocation = WindowStartupLocation.CenterScreen,
            Title = "Vantage Setup", Icon = Mark(), SnapsToDevicePixels = true,
        };
        root.MouseLeftButtonDown += delegate (object s, MouseButtonEventArgs e) { try { window.DragMove(); } catch { } };
        Find<Button>("CloseBtn").Click += delegate { if (Cancel != null) Cancel.Cancel = true; window.Close(); };
        Find<Button>("MinBtn").Click += delegate { window.WindowState = WindowState.Minimized; };
        Find<Button>("ChangeFolder").Click += delegate { ChooseFolder(); };
        window.Loaded += delegate {
            if (action != null) Begin(action);
            else if (product != null) ShowMaintain();
            else ShowWelcome();
        };
        app.Run(window);
        return 0;
    }
}
