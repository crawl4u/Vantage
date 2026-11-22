# Compiles the setup program: Setup.cs and Setup.xaml with the MSI, logo and version embedded.
# Needs only the .NET SDK's Roslyn compiler and Windows' own WPF assemblies; nothing is downloaded.
# Usage: build-setup.ps1 -Msi <file.msi> -Version <x.y.z> -Icon <file.ico> -Out <setup.exe>
param([Parameter(Mandatory)][string]$Msi, [Parameter(Mandatory)][string]$Version,
      [Parameter(Mandatory)][string]$Icon, [Parameter(Mandatory)][string]$Out)
$ErrorActionPreference = "Stop"
$fw = Join-Path $env:WINDIR "Microsoft.NET\Framework64\v4.0.30319"
$roslyn = Get-ChildItem (Join-Path $env:ProgramFiles "dotnet\sdk") -Recurse -Filter csc.dll |
    Where-Object { $_.FullName -match 'Roslyn\\bincore' } | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $roslyn) { throw "the .NET SDK (Roslyn csc.dll) is required to build setup" }
$work = Join-Path ([IO.Path]::GetTempPath()) ("vantage-setup-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory $work | Out-Null
try {
    Set-Content -LiteralPath (Join-Path $work "version.txt") -Value $Version -NoNewline
    $refs = "mscorlib", "System", "System.Core", "System.Xml", "System.Xaml", "System.Windows.Forms", "WPF\PresentationCore",
            "WPF\PresentationFramework", "WPF\WindowsBase" | ForEach-Object { "/r:$fw\$_.dll" }
    $res = @(
        "/resource:$((Resolve-Path (Join-Path $PSScriptRoot 'Setup.xaml')).Path),Setup.xaml",
        "/resource:$((Resolve-Path (Join-Path $PSScriptRoot 'mark.png')).Path),mark.png",
        "/resource:$((Resolve-Path $Msi).Path),Vantage.msi",
        "/resource:$(Join-Path $work 'version.txt'),version.txt")
    $out = [IO.Path]::GetFullPath($Out)
    dotnet $roslyn.FullName /nologo /noconfig /nostdlib+ /target:winexe /platform:x64 /optimize+ "/out:$out" `
        "/win32icon:$((Resolve-Path $Icon).Path)" "/win32manifest:$(Join-Path $PSScriptRoot '..\as-invoker.manifest')" `
        @refs @res (Join-Path $PSScriptRoot "Setup.cs")
    if ($LASTEXITCODE -ne 0) { throw "setup compile failed" }
} finally { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue }
