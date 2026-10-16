#!/usr/bin/env bash
set -euo pipefail

target=${1:?Rust target required}
version=${2:?Release version required}
root=$(pwd)
name=mm
extension=tar.gz
if [[ "$target" == *-windows-* ]]; then
  name=mm.exe
  extension=zip
fi
binary="$root/target/$target/release/$name"
actual=$("$binary" --version)
if [[ "$version" != preview-release ]]; then
  if [[ ! "$version" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    printf 'Release tags must use vMAJOR.MINOR.PATCH: %s\n' "$version" >&2
    exit 1
  fi
  if [[ "$actual" != "mm ${version#v}" ]]; then
    printf 'Tag %s does not match binary version %s\n' "$version" "$actual" >&2
    exit 1
  fi
fi
"$binary" --help >/dev/null

stage=$(mktemp -d)
trap 'rm -f "$stage/mm" "$stage/mm.exe" "$stage/LICENSE" "$stage/README.md"; rmdir "$stage"' EXIT
cp "$binary" "$stage/$name"
cp LICENSE README.md "$stage/"
mkdir -p "$root/dist"
archive="markmason-$version-$target.$extension"
if [[ "$extension" == zip ]]; then
  (cd "$stage" && 7z a -tzip "$root/dist/$archive" "$name" LICENSE README.md >/dev/null)
else
  tar -czf "$root/dist/$archive" -C "$stage" "$name" LICENSE README.md
fi
(
  cd "$root/dist"
  if command -v sha256sum >/dev/null; then
    sha256sum "$archive" >"$archive.sha256"
  else
    shasum -a 256 "$archive" >"$archive.sha256"
  fi
)
printf 'Packaged %s (%s)\n' "$archive" "$actual"
