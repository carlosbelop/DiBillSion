#!/usr/bin/env bash
# Builds dibill-core for iOS and writes ios/Generated/: DibillCore.xcframework (static
# library + C header + module map) and the UniFFI Swift bindings (dibill_core.swift).
# macOS only (Xcode). Usage:
#   scripts/build-xcframework.sh           # device + simulator, `mobile` profile (release)
#   scripts/build-xcframework.sh --sim     # simulator only, debug (fast; CI tests)
#   scripts/build-xcframework.sh --device  # device only, `mobile` profile (archives for TestFlight)
set -euo pipefail
cd "$(dirname "$0")/.."

profile=mobile
targets=(aarch64-apple-ios aarch64-apple-ios-sim)
case "${1:-}" in
  --sim) profile=dev; targets=(aarch64-apple-ios-sim) ;;
  --device) targets=(aarch64-apple-ios) ;;
  "") ;;
  *) echo "usage: $0 [--sim|--device]" >&2; exit 2 ;;
esac
out=ios/Generated
export IPHONEOS_DEPLOYMENT_TARGET=17.0

for t in "${targets[@]}"; do
  cargo build -p dibill-core --lib --locked --profile "$profile" --target "$t"
done

# Bindings come from the host build's metadata; they are the same for every target.
scripts/gen-bindings.sh target/bindings/swift swift
rm -rf "$out" && mkdir -p "$out/include"
cp target/bindings/swift/dibill_core.swift "$out/"
cp target/bindings/swift/dibill_coreFFI.h "$out/include/"
cp target/bindings/swift/dibill_coreFFI.modulemap "$out/include/module.modulemap"

dir=$([[ $profile == dev ]] && echo debug || echo "$profile")
args=()
for t in "${targets[@]}"; do
  args+=(-library "target/$t/$dir/libdibill_core.a" -headers "$out/include")
done
xcodebuild -create-xcframework "${args[@]}" -output "$out/DibillCore.xcframework"
echo "Wrote $out (profile $profile: ${targets[*]})"
