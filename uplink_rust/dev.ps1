<#
Local development stack for uplink_rust on Windows.

  .\dev.ps1               account server + the desktop game
  .\dev.ps1 -Web          account server + the browser version (http://127.0.0.1:8765)
  .\dev.ps1 -ServerOnly   account server, nothing else
  .\dev.ps1 -Stop         shut the server and web page down

The account server runs on http://127.0.0.1:8080 with the built-in "Test account" sign-in
(UPLINK_RUST_DEV_LOGIN: local testing only, anyone who reaches it can sign in as anyone).

Needs, once:
  - an empty PostgreSQL database, and its address in .dev\database.url (git-ignored), e.g.
    postgres://USER:PASSWORD@127.0.0.1:5432/uplink_rust
  - for -Web: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli (same
    version as the wasm-bindgen crate in Cargo.lock); Python for the static file server.
#>
param([switch]$Web, [switch]$Stop, [switch]$ServerOnly)
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$dev = Join-Path $root '.dev'

function Listening([int]$port) {
    [bool](Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue)
}

if ($Stop) {
    foreach ($port in 8080, 8765) {
        Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue |
            ForEach-Object { Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue }
    }
    'Stopped.'
    return
}

# 1. Account server
$urlFile = Join-Path $dev 'database.url'
if (-not (Test-Path $urlFile)) { throw "Put your PostgreSQL address in $urlFile (see the top of this script)." }
$keyFile = Join-Path $dev 'server.key'
if (-not (Test-Path $keyFile)) {
    # Seals the verified runs in the database; a new key makes existing ones unreadable.
    $b = New-Object byte[] 32
    [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($b)
    ($b | ForEach-Object { $_.ToString('x2') }) -join '' | Set-Content $keyFile -NoNewline
}
if (-not (Listening 8080)) {
    cargo build --release -p ur-server --manifest-path "$root\Cargo.toml"
    if ($LASTEXITCODE -ne 0) { throw 'server build failed' }
    $env:DATABASE_URL = (Get-Content $urlFile -Raw).Trim()
    $env:UPLINK_RUST_SERVER_KEY = (Get-Content $keyFile -Raw).Trim()
    $env:UPLINK_RUST_PUBLIC_URL = 'http://127.0.0.1:8080'
    $env:UPLINK_RUST_DEV_LOGIN = '1'
    $env:UPLINK_RUST_WEB_ORIGINS = 'http://127.0.0.1:8765'
    $env:UPLINK_RUST_ALLOWED_ORIGIN = 'http://127.0.0.1:8765'
    $env:BIND = '127.0.0.1:8080'
    Start-Process "$root\target\release\ur-server.exe" -WindowStyle Hidden `
        -RedirectStandardOutput "$dev\server.log" -RedirectStandardError "$dev\server.err.log"
    foreach ($i in 1..40) { if (Listening 8080) { break }; Start-Sleep -Milliseconds 250 }
    if (-not (Listening 8080)) { throw "the account server did not start; see $dev\server.err.log" }
}
'Account server is running on port 8080 (the game talks to it; it has no page to open).'
if ($ServerOnly) { return }

# 2. The game
if ($Web) {
    cargo build -p ur-client --lib --target wasm32-unknown-unknown --profile web --manifest-path "$root\Cargo.toml"
    if ($LASTEXITCODE -ne 0) { throw 'web build failed' }
    $src = Join-Path $root 'crates\ur-client\web'
    wasm-bindgen --target web --no-typescript --out-dir "$src\pkg" "$root\target\wasm32-unknown-unknown\web\ur_client.wasm"
    if ($LASTEXITCODE -ne 0) { throw 'wasm-bindgen failed' }
    # Serve a copy whose page points at the local account server; the committed page stays neutral.
    $site = Join-Path $dev 'web'
    New-Item -ItemType Directory -Force $site | Out-Null
    Copy-Item "$src\*" $site -Recurse -Force
    $page = Join-Path $site 'index.html'
    (Get-Content $page -Raw).Replace('name="uplink-rust-server" content=""',
        'name="uplink-rust-server" content="http://127.0.0.1:8080"') | Set-Content $page -NoNewline
    if (-not (Listening 8765)) {
        Start-Process python -ArgumentList "`"$root\dev-web.py`" `"$site`"" -WindowStyle Hidden
    }
    'Browser version: http://127.0.0.1:8765  (pick the .dat files from uplink\bin when asked)'
    Start-Process 'http://127.0.0.1:8765/'
} else {
    $env:UPLINK_RUST_SERVER = 'http://127.0.0.1:8080'
    if (-not $env:UPLINK_DIR) { $env:UPLINK_DIR = (Resolve-Path "$root\..\uplink\bin").Path }
    cargo run --release -p ur-client --manifest-path "$root\Cargo.toml"
}
