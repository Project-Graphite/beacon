$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$app = Join-Path $root 'app'
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
$releaseDir = Join-Path $targetDir 'release'
$dist = Join-Path $root 'dist'

function Invoke-Checked {
    param([scriptblock]$Command)
    & $Command
    if ($LASTEXITCODE -ne 0) { throw "command failed with exit code ${LASTEXITCODE}: $Command" }
}

if (Invoke-Checked { git -C $root status --porcelain }) {
    throw 'the working tree has uncommitted or untracked changes: commit or stash them before a release'
}

$version = (Get-Content (Join-Path $app 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json).version

Push-Location $root
try {
    Invoke-Checked { cargo build --release --locked -p beacon-cli -p beacon-hook }
}
finally {
    Pop-Location
}

$resources = @{
    bundle = @{
        resources = @{
            (Join-Path $releaseDir 'beacon.exe') = 'beacon.exe'
            (Join-Path $releaseDir 'beacon-hook.exe') = 'beacon-hook.exe'
        }
    }
}
$resourcesConfig = Join-Path ([IO.Path]::GetTempPath()) "beacon-release-$PID.json"
[IO.File]::WriteAllText($resourcesConfig, ($resources | ConvertTo-Json -Depth 5))

Push-Location $app
try {
    if (-not (Test-Path 'node_modules')) {
        Invoke-Checked { npm ci }
    }
    Invoke-Checked { npx tauri build --config $resourcesConfig -- --locked }
}
finally {
    Pop-Location
    Remove-Item $resourcesConfig -ErrorAction SilentlyContinue
}

$setup = Join-Path $releaseDir "bundle\nsis\Beacon_${version}_x64-setup.exe"
if (-not (Test-Path $setup)) { throw "installer not found: $setup" }

if (Test-Path $dist) { Remove-Item $dist -Recurse -Force }
New-Item -ItemType Directory $dist | Out-Null

Copy-Item $setup (Join-Path $dist "Beacon-$version-setup.exe")

$staging = Join-Path $dist "beacon-$version-windows-x64"
New-Item -ItemType Directory $staging | Out-Null
foreach ($file in 'beacon-app.exe', 'beacon.exe', 'beacon-hook.exe') {
    Copy-Item (Join-Path $releaseDir $file) $staging
}
Copy-Item (Join-Path $root 'LICENSE'), (Join-Path $root 'README.md') $staging
Compress-Archive -Path (Join-Path $staging '*') -DestinationPath (Join-Path $dist "beacon-$version-windows-x64.zip")
Remove-Item $staging -Recurse -Force

$sums = Get-ChildItem $dist -File | Sort-Object Name | ForEach-Object {
    $hash = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $($_.Name)"
}
Set-Content (Join-Path $dist 'SHA256SUMS.txt') $sums -Encoding ascii

Get-ChildItem $dist -File | Select-Object Name, Length
