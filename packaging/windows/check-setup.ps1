# Extracts the MSI embedded in the setup program, without installing, and requires it to be the checked MSI.
param([Parameter(Mandatory)][string]$Setup, [Parameter(Mandatory)][string]$Msi,
      [Parameter(Mandatory)][string]$Out, [string]$Wix = "wix")
$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "execution-level.ps1") -Executable $Setup
if (Test-Path -LiteralPath $Out) { throw "Setup extraction output already exists: $Out" }
Start-Process -FilePath $Setup -ArgumentList "--extract", "`"$Out`"" -Wait -NoNewWindow
$files = @(Get-ChildItem -LiteralPath $Out -Recurse -File)
if ($files.Count -ne 1) { throw "Setup must embed exactly one MSI package" }
if ((Get-FileHash -LiteralPath $files[0].FullName -Algorithm SHA256).Hash -ne
    (Get-FileHash -LiteralPath $Msi -Algorithm SHA256).Hash) { throw "Setup contains a different MSI" }
Write-Output "Setup embeds only the verified MSI"
