# Publishes an update. Everyone with the app gets it automatically.
#
#   .\release.ps1 2.0.1 "Fixed something"
#
# Needs: GitHub CLI signed in (gh auth login) and the update key in %USERPROFILE%\.duet-release (shared with Duet).
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$Notes = ""
)
$ErrorActionPreference = "Continue"
Set-Location $PSScriptRoot
$repo = "Hydrate36/Genshin-Wishing-Calculator"

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Version must look like 2.0.1" }
gh release view "v$Version" -R $repo 2>&1 | Out-Null
if ($LASTEXITCODE -eq 0) { throw "v$Version is already released. Pick a higher number." }
$key = "$env:USERPROFILE\.duet-release\updater.key"
if (!(Test-Path $key)) { throw "Update key missing: $key" }
Get-Process wish-tracker -ErrorAction SilentlyContinue | Stop-Process -Force

$utf8 = New-Object System.Text.UTF8Encoding($false)
foreach ($f in "src-tauri\tauri.conf.json", "package.json") {
    [IO.File]::WriteAllText((Resolve-Path $f), ((Get-Content $f -Raw) -replace '"version": "[^"]+"', "`"version`": `"$Version`""), $utf8)
}
$cargo = "src-tauri\Cargo.toml"
[IO.File]::WriteAllText((Resolve-Path $cargo), ((Get-Content $cargo -Raw) -replace '(?m)^version = "[^"]+"', "version = `"$Version`""), $utf8)

$env:Path += ";$env:USERPROFILE\.cargo\bin"
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content $key -Raw).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
npx tauri build --bundles nsis
if ($LASTEXITCODE -ne 0) { throw "Build failed" }

$dir = "src-tauri\target\release\bundle\nsis"
$exeName = "Genshin Wishing Calculator_${Version}_x64-setup.exe"
$exe = Join-Path $dir $exeName
if (!(Test-Path "$exe.sig")) { throw "No update signature was made ($exe.sig)" }
$upName = "Genshin-Wishing-Calculator_${Version}_x64-setup.exe"
$up = Join-Path $dir $upName
Copy-Item $exe $up -Force

$latest = [ordered]@{
    version   = $Version
    notes     = $Notes
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = @{ "windows-x86_64" = @{ signature = (Get-Content "$exe.sig" -Raw).Trim(); url = "https://github.com/$repo/releases/download/v$Version/$upName" } }
}
[IO.File]::WriteAllText((Join-Path (Resolve-Path $dir) "latest.json"), ($latest | ConvertTo-Json -Depth 5), $utf8)

git add -A
git -c user.name=Hydrate36 -c user.email=Hydrate36@users.noreply.github.com commit -q -m "Version $Version" 2>&1 | Out-Null
git push -q origin HEAD --tags
if ($LASTEXITCODE -ne 0) { throw "git push failed" }

if (!$Notes) { $Notes = "Genshin Wishing Calculator $Version" }
gh release create "v$Version" $up (Join-Path $dir "latest.json") -R $repo --title "Genshin Wishing Calculator $Version" --notes $Notes
if ($LASTEXITCODE -ne 0) { throw "Upload failed" }
Write-Host "https://github.com/$repo/releases/tag/v$Version"
