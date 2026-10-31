#!/usr/bin/env bash
# Generates the UniFFI bindings for dibill-core.
# Usage: scripts/gen-bindings.sh [out-dir] [kotlin|swift]   (default: target/bindings/<language>, kotlin)
# The Android build calls this through Gradle (android/app/build.gradle.kts);
# scripts/build-xcframework.sh calls it for Swift.
set -euo pipefail
cd "$(dirname "$0")/.."
language=${2:-kotlin}
out=${1:-target/bindings/$language}

cargo build -q -p dibill-core --lib
case "$(uname -s)" in
  Darwin) lib=target/debug/libdibill_core.dylib ;;
  *) lib=target/debug/libdibill_core.so ;;
esac

rm -rf "$out"
cargo run -q -p dibill-core --features bindgen --bin uniffi-bindgen -- \
  generate --library "$lib" --language "$language" --out-dir "$out" --no-format
echo "${language^} bindings in $out"
