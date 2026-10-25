#!/usr/bin/env bash
# Generates the Kotlin (and, on macOS later, Swift) bindings for dibill-core.
# Usage: scripts/gen-bindings.sh [out-dir]   (default: target/bindings)
set -euo pipefail
cd "$(dirname "$0")/.."
out=${1:-target/bindings}

cargo build -p dibill-core --lib
case "$(uname -s)" in
  Darwin) lib=target/debug/libdibill_core.dylib ;;
  *) lib=target/debug/libdibill_core.so ;;
esac

cargo run -q -p dibill-core --features bindgen --bin uniffi-bindgen -- \
  generate --library "$lib" --language kotlin --out-dir "$out/kotlin" --no-format
echo "Kotlin bindings in $out/kotlin"
