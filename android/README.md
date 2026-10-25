# Android app

Jetpack Compose shell. Created in milestone M3 with Android Studio.
The Rust core is built with `cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o app/src/main/jniLibs build --profile mobile -p dibill-core`.
