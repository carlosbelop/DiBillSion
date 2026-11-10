#!/usr/bin/env bash
# Builds the signed release APK for testers and copies it to an output folder, named by version.
# Needs the owner's signing key (Gradle property dibill.releaseSigning) and the server URL
# (dibill.syncUrl), both outside the repo. Build from an up-to-date main: the version code is
# the commit count, so each new build installs over the previous one.
#
#   scripts/android-release.sh /mnt/c/Users/me/DiBillSion-releases
set -euo pipefail

out="${1:?usage: scripts/android-release.sh <output folder>}"
root="$(git rev-parse --show-toplevel)"
apk="$root/android/app/build/outputs/apk/release/app-release.apk"

(cd "$root/android" && ./gradlew -q :app:assembleRelease)
[ -f "$apk" ] || { echo "no signed APK: set dibill.releaseSigning (see android/README.md)" >&2; exit 1; }

sdk="${ANDROID_HOME:-$HOME/Android/Sdk}"
tools="$(ls -d "$sdk"/build-tools/* | sort -V | tail -1)"
"$tools/apksigner" verify "$apk"
version="$("$tools/aapt2" dump badging "$apk" | sed -n "s/.*versionName='\([^']*\)'.*/\1/p")"
url="$(grep -o 'SYNC_URL = "[^"]*"' "$root/android/app/build/generated/source/buildConfig/release/com/dibillsion/app/BuildConfig.java" | cut -d'"' -f2)"
[ -n "$url" ] || { echo "the release build has no server: set dibill.syncUrl" >&2; exit 1; }

mkdir -p "$out"
cp "$apk" "$out/DiBillSion-$version.apk"
echo "DiBillSion $version → $out/DiBillSion-$version.apk (server $url)"
