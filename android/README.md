# Android app

Jetpack Compose shell (Material 3) on top of the Rust core (`dibill-core`) through UniFFI.

```bash
cd android
./gradlew :app:assembleDebug      # builds the Rust core with cargo-ndk and generates the bindings
./gradlew :app:testDebugUnitTest
```

Needs `ANDROID_HOME` / `ANDROID_NDK_HOME` and `cargo-ndk` (all installed by `scripts/setup-ubuntu.sh`).
The Gradle build runs two tasks per variant:

- `cargoNdk<Variant>`: `cargo ndk` for arm64-v8a, armeabi-v7a and x86_64 (`dev` profile for debug,
  `mobile` for release) into `build/generated/rustJniLibs/<variant>`.
- `uniffiBindings<Variant>`: `scripts/gen-bindings.sh` into `build/generated/uniffi/<variant>`.

Neither output is checked in.
