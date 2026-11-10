#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
name="minicraft-v${version}-windows-x64"
output="../releases/v${version}"
if [[ -e "${output}/${name}.exe" ]]; then
  echo "Release v${version} already exists. Bump Cargo.toml before packaging a new build." >&2
  exit 1
fi
cargo build --locked --release --target x86_64-pc-windows-gnu
mkdir -p "$output"
cp target/x86_64-pc-windows-gnu/release/minicraft.exe "${output}/${name}.exe"
cp WINDOWS-README.txt "${output}/README.txt"
cp assets/FONT-LICENSE.txt "${output}/FONT-LICENSE.txt"
sha256sum "${output}/${name}.exe" | sed "s|${output}/||" > "${output}/SHA256SUMS.txt"
python3 - "$output" "$name" <<'PY'
import pathlib,sys,zipfile
output=pathlib.Path(sys.argv[1]); name=sys.argv[2]
with zipfile.ZipFile(output / f'{name}.zip','w',zipfile.ZIP_DEFLATED) as archive:
 for file in [f'{name}.exe','README.txt','SHA256SUMS.txt','FONT-LICENSE.txt']:
  archive.write(output / file,file)
PY
printf 'Packaged %s/%s.exe\n' "$output" "$name"
