#!/usr/bin/env bash
# Verifies downloaded payloads, archive permissions, licenses and an actual CLI search.
set -euo pipefail

directory=$(mktemp -d)
trap 'rm -rf "$directory"' EXIT
for platform in linux-x86_64 macos-aarch64 windows-x86_64; do
  sha256sum --check "artifact-sha256sums-${platform}.txt"
  mkdir "$directory/$platform"
  tar -xzf "asearch-assets-${platform}.tar.gz" -C "$directory/$platform"
  extension=
  if [ "$platform" = windows-x86_64 ]; then extension=.exe; fi
  cmp "$directory/$platform/asearch-cli${extension}" "asearch-cli-${platform}${extension}"
  for notice in LICENSE-MIT LICENSE-APACHE NOTICE licenses/Unicode-3.0.txt licenses/xxhash-rust.txt; do
    test -s "$directory/$platform/$notice"
  done
  test -d "$directory/$platform/licenses/server-dependencies"
  test -d "$directory/$platform/licenses/rust-dependencies"
  header=$(od -An -tx1 -N8 "$directory/$platform/asearch_wasm.wasm" | tr -d '[:space:]')
  test "$header" = 0061736d01000000
done

cmp "$directory/linux-x86_64/asearch_wasm.wasm" asearch_wasm.wasm
mkdir "$directory/licenses"
tar -xzf asearch-licenses.tar.gz -C "$directory/licenses"
cmp "$directory/licenses/NOTICE" "$directory/linux-x86_64/NOTICE"

cli="$directory/linux-x86_64/asearch-cli"
test -x "$cli"
printf '%s\n' '{"key":7,"texts":["artifact roundtrip"]}' > "$directory/docs.jsonl"
"$cli" build "$directory/docs.jsonl" "$directory/base.asix"
"$cli" search "$directory/base.asix" artifact > "$directory/results.json"
jq -e '.hits | length == 1 and .[0].key == 7 and .[0].level == 0' "$directory/results.json"
