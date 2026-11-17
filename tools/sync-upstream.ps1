# Pulls the newest upstream Cinnabar work into this fork on a separate branch, without
# touching main or pushing anything. Usage: tools/sync-upstream.ps1 [-Upstream dev]
#
# Vantage's own code (crates/vantage, app/src/modding/vantage, vantage-mod) does not exist
# upstream, so it never conflicts. Branding, README, FORK.md and release-assets.json are
# kept as ours (see .gitattributes). Any other conflict is reported and the merge undone.
param([string]$Upstream = "dev")
$ErrorActionPreference = "Stop"
Set-Location (Resolve-Path (Join-Path $PSScriptRoot ".."))
if (git status --porcelain) { throw "commit or stash your changes first" }
if (-not (git remote | Select-String -SimpleMatch -Quiet "upstream")) {
    git remote add upstream https://github.com/cinnabar-mc/cinnabar.git
}
git config merge.ours.driver true
git fetch upstream
$behind = [int](git rev-list --count "HEAD..upstream/$Upstream")
if ($behind -eq 0) { "Already up to date with upstream/$Upstream."; return }
$main = git branch --show-current
$branch = "sync/upstream-" + (Get-Date -Format "yyyyMMdd-HHmm")
git checkout -b $branch
git merge --no-edit "upstream/$Upstream"
if ($LASTEXITCODE -ne 0) {
    $conflicts = git diff --name-only --diff-filter=U
    git merge --abort
    git checkout $main
    git branch -D $branch | Out-Null
    "Merge undone: upstream changed lines we also changed. Files:"
    $conflicts | ForEach-Object { "  $_" }
    "Ask Claude to resolve these, or merge by hand: git merge upstream/$Upstream"
    exit 1
}
"Merged $behind upstream commits on branch $branch."
"Next: cargo build --profile play -p cinnabar, test, then 'git checkout $main; git merge $branch'."
"If the mod API changed, diff crates/mod-api/wit/extension.wit and adapt crates/vantage."
