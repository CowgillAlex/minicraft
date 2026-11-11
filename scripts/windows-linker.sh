#!/usr/bin/env bash
set -euo pipefail
# meshopt requests stdc++ through Cargo metadata. Resolve that one library to
# its static archive so the versioned download needs no C++ runtime DLL.
cpp_archive="$(x86_64-w64-mingw32-g++-posix -print-file-name=libstdc++.a)"
link_args=()
for argument in "$@"; do
  if [[ "$argument" == '-lstdc++' ]]; then
    link_args+=("$cpp_archive")
  else
    link_args+=("$argument")
  fi
done
exec x86_64-w64-mingw32-gcc "${link_args[@]}"
