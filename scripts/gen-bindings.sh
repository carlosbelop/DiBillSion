#!/usr/bin/env bash
# Generates the Kotlin bindings for dibill-core (Swift comes with M4 on macOS).
# Usage: scripts/gen-bindings.sh [out-dir]   (default: target/bindings/kotlin)
# The Android build calls this through Gradle; see android/app/build.gradle.kts.
set -euo pipefail
cd "$(dirname "$0")/.."
out=${1:-target/bindings/kotlin}

cargo build -q -p dibill-core --lib
case "$(uname -s)" in
  Darwin) lib=target/debug/libdibill_core.dylib ;;
  *) lib=target/debug/libdibill_core.so ;;
esac

rm -rf "$out"
cargo run -q -p dibill-core --features bindgen --bin uniffi-bindgen -- \
  generate --library "$lib" --language kotlin --out-dir "$out" --no-format
echo "Kotlin bindings in $out"
