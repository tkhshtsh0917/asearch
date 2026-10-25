#!/usr/bin/env bash
# Builds the platform's release payload and records every transported file hash.
set -euo pipefail

platform=${1:?platform required}
extension=${2-}
case "$platform:$extension" in
  linux-x86_64:|macos-aarch64:|windows-x86_64:.exe) ;;
  *) echo "Unsupported platform/extension" >&2; exit 1 ;;
esac

identity=$(cargo pkgid -p asearch-engine)
version=${identity##*#}
version=${version##*@}
if [ "${GITHUB_REF_TYPE:-branch}" = tag ]; then
  test "$GITHUB_REF_NAME" = "v${version}"
fi

export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=${GITHUB_WORKSPACE:-$PWD}=/workspace/$(printf '\037')--remap-path-prefix=${HOME}=/build/home$(printf '\037')--remap-path-prefix=${CARGO_HOME:-${USERPROFILE:-$HOME}/.cargo}=/build/cargo$(printf '\037')--remap-path-prefix=$(rustc --print sysroot)=/build/rust"
cargo build --locked --release -p asearch-cli
cargo build --locked --release -p asearch-wasm --target wasm32-unknown-unknown

mkdir -p bundle/licenses/rust-dependencies bundle/licenses/server-dependencies
cp target/wasm32-unknown-unknown/release/asearch_wasm.wasm bundle/asearch_wasm.wasm
cp "target/release/asearch-cli${extension}" bundle/
cp LICENSE-MIT LICENSE-APACHE NOTICE bundle/
cp crates/asearch-engine/LICENSE-xxhash-rust.txt bundle/licenses/xxhash-rust.txt
cp -R crates/asearch-engine/licenses/third-party/. bundle/licenses/rust-dependencies/
cp -R crates/asearch-analyzer/licenses/third-party/. bundle/licenses/server-dependencies/
cp crates/asearch-engine/LICENSE-Unicode-3.0.txt bundle/licenses/Unicode-3.0.txt
tar -czf "asearch-assets-${platform}.tar.gz" -C bundle .
cp "bundle/asearch-cli${extension}" "asearch-cli-${platform}${extension}"

files=("asearch-assets-${platform}.tar.gz" "asearch-cli-${platform}${extension}")
if [ "$platform" = linux-x86_64 ]; then
  cp bundle/asearch_wasm.wasm .
  tar -czf asearch-licenses.tar.gz -C bundle LICENSE-MIT LICENSE-APACHE NOTICE licenses
  files+=(asearch_wasm.wasm asearch-licenses.tar.gz)
fi
for file in "${files[@]}"; do
  openssl dgst -sha256 -r "$file"
done > "artifact-sha256sums-${platform}.txt"
