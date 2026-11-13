$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    $manifest = Get-Content -LiteralPath 'Cargo.toml' -Raw
    $version = [regex]::Match($manifest, '(?m)^version = "([^"]+)"').Groups[1].Value
    if (-not $version) { throw 'Could not read the package version from Cargo.toml.' }
    $name = "minicraft-v$version-windows-x64"
    $output = Join-Path 'releases' "v$version"
    $executable = Join-Path $output "$name.exe"
    if (Test-Path -LiteralPath $output) {
        throw "Release directory $output already exists. Bump Cargo.toml before packaging a new build."
    }

    cargo build --locked --release --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Windows release build failed.' }

    New-Item -ItemType Directory -Path $output | Out-Null
    Copy-Item -LiteralPath 'target/x86_64-pc-windows-msvc/release/minicraft.exe' -Destination $executable
    Copy-Item -LiteralPath 'WINDOWS-README.txt' -Destination (Join-Path $output 'README.txt')
    Copy-Item -LiteralPath 'assets/FONT-LICENSE.txt' -Destination $output
    Copy-Item -LiteralPath 'assets/THIRD-PARTY-LICENSES.txt' -Destination $output
    $hash = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $name.exe" | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding ascii
    $files = @("$name.exe", 'README.txt', 'FONT-LICENSE.txt', 'THIRD-PARTY-LICENSES.txt', 'SHA256SUMS.txt') |
        ForEach-Object { Join-Path $output $_ }
    Compress-Archive -LiteralPath $files -DestinationPath (Join-Path $output "$name.zip")
    Write-Host "Packaged $executable"
} finally {
    Pop-Location
}
