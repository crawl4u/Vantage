# Runs a setup exe on a private, invisible desktop and saves a picture of each window it shows,
# so its look can be checked without a window ever appearing on the real screen. Nothing is
# installed: -ClickAfter names one button (such as "&Options") to press before a second picture.
# Usage: shoot-setup.ps1 -Exe <setup.exe> -OutPrefix <path\prefix> [-ClickAfter <button text>]
param([Parameter(Mandatory)][string]$Exe, [Parameter(Mandatory)][string]$OutPrefix, [string]$ClickAfter = "")
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
public static class Shoot {
    public static List<string> Log = new List<string>();
    [StructLayout(LayoutKind.Sequential)] struct RECT { public int L, T, R, B; }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)] struct STARTUPINFO {
        public int cb; public string lpReserved, lpDesktop, lpTitle; public int dwX, dwY, dwXSize, dwYSize,
        dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags; public short wShowWindow, cbReserved2;
        public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError; }
    [StructLayout(LayoutKind.Sequential)] struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public int dwProcessId, dwThreadId; }
    delegate bool EnumProc(IntPtr hwnd, IntPtr lparam);
    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)] static extern IntPtr CreateDesktop(string name, IntPtr device, IntPtr devmode, int flags, uint access, IntPtr sa);
    [DllImport("user32.dll")] static extern bool CloseDesktop(IntPtr h);
    [DllImport("user32.dll")] static extern bool SetThreadDesktop(IntPtr h);
    [DllImport("user32.dll")] static extern bool EnumDesktopWindows(IntPtr desktop, EnumProc proc, IntPtr lparam);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent, EnumProc proc, IntPtr lparam);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
    [DllImport("user32.dll")] static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)] static extern bool CreateProcess(string app, string cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags, IntPtr env, string dir, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
    [DllImport("kernel32.dll")] static extern bool TerminateProcess(IntPtr h, uint code);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);

    static List<IntPtr> Top(IntPtr desktop) {
        var found = new List<IntPtr>();
        EnumDesktopWindows(desktop, (h, l) => { if (IsWindowVisible(h)) found.Add(h); return true; }, IntPtr.Zero);
        return found;
    }
    static string Text(IntPtr h) { var s = new StringBuilder(256); GetWindowText(h, s, 256); return s.ToString(); }
    static void Save(IntPtr h, string path) {
        RECT r; GetWindowRect(h, out r);
        int w = r.R - r.L, hh = r.B - r.T;
        if (w <= 0 || hh <= 0) return;
        using (var bmp = new Bitmap(w, hh))
        using (var g = Graphics.FromImage(bmp)) {
            IntPtr dc = g.GetHdc(); PrintWindow(h, dc, 2); g.ReleaseHdc(dc);
            bmp.Save(path, ImageFormat.Png);
        }
        Log.Add("saved " + path + " " + w + "x" + hh + " '" + Text(h) + "'");
    }
    public static void Run(string exe, string prefix, string click) {
        Exception failure = null;
        var worker = new Thread(() => {
            try {
                IntPtr desktop = CreateDesktop("vantage_preview", IntPtr.Zero, IntPtr.Zero, 0, 0x10000000, IntPtr.Zero);
                if (desktop == IntPtr.Zero) throw new Exception("no desktop " + Marshal.GetLastWin32Error());
                var si = new STARTUPINFO(); si.cb = Marshal.SizeOf(si); si.lpDesktop = "winsta0\\vantage_preview";
                PROCESS_INFORMATION pi;
                if (!CreateProcess(exe, "\"" + exe + "\"", IntPtr.Zero, IntPtr.Zero, false, 0, IntPtr.Zero, null, ref si, out pi))
                    throw new Exception("start failed " + Marshal.GetLastWin32Error());
                try {
                    Thread.Sleep(5000);
                    if (!SetThreadDesktop(desktop)) throw new Exception("SetThreadDesktop " + Marshal.GetLastWin32Error());
                    var tops = Top(desktop); Log.Add("windows: " + tops.Count + " alive=" + !Process.GetProcessById(pi.dwProcessId).HasExited);
                    int n = 0;
                    foreach (var h in tops) if (Text(h).Length > 0 || true) { Save(h, prefix + "_" + (n++) + ".png"); }
                    if (click.Length > 0) {
                        foreach (var h in tops) {
                            EnumChildWindows(h, (c, l) => { if (Text(c) == click) { SendMessage(c, 0x00F5, IntPtr.Zero, IntPtr.Zero); } return true; }, IntPtr.Zero);
                        }
                        Thread.Sleep(2500);
                        n = 0;
                        foreach (var h in Top(desktop)) Save(h, prefix + "_after_" + (n++) + ".png");
                    }
                } finally { TerminateProcess(pi.hProcess, 0); CloseHandle(pi.hProcess); CloseHandle(pi.hThread); CloseDesktop(desktop); }
            } catch (Exception e) { failure = e; }
        });
        worker.SetApartmentState(ApartmentState.STA);
        worker.Start(); worker.Join();
        if (failure != null) throw failure;
    }
}
'@
[Shoot]::Run((Resolve-Path $Exe).Path, $OutPrefix, $ClickAfter)
[Shoot]::Log
