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

## Builds for testers

Two apps that can live side by side on one phone:

| Build | Package | Name on the phone | Server |
|---|---|---|---|
| debug | `com.dibillsion.app.dev` | DiBillSion dev | `dibill.debugSyncUrl`, else `http://127.0.0.1:8080` (`adb reverse`) |
| release | `com.dibillsion.app` | DiBillSion | `dibill.syncUrl` (or `DIBILL_SYNC_URL`) |

Everything private lives outside the repo, in `~/.gradle/gradle.properties`:

```properties
dibill.syncUrl=https://api.example.com
dibill.debugSyncUrl=https://api.example.com
dibill.releaseSigning=/home/<you>/.config/dibillsion/android-release.properties
```

The signing properties file holds `storeFile`, `storePassword`, `keyAlias` and `keyPassword`.
**The keystore can't be replaced:** an APK signed with another key won't install over the
testers' app (they would have to uninstall and lose their local data). Keep a copy of the
`.jks` and its properties file somewhere safe, such as a password manager.

`scripts/android-release.sh <folder>` builds the signed APK from the current checkout (build from
an up-to-date `main`), checks the signature and the server URL, and writes
`DiBillSion-0.1.<commits>.apk` there. The version code is the commit count, so each new build
installs over the last one and keeps the testers' data.

Testers install it from the file (Android asks to allow installs from that app once). Updates
are the same: open the newer APK.
