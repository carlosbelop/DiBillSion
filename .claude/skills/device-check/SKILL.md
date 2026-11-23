---
name: device-check
description: Try a DiBillSion Android change on the owner's Pixel 9a over adb wireless debugging - install the dev build, drive the UI, take screenshots, check Spanish and dark mode, read logs, test a receipt scan. Use after UI changes, before calling a UI PR done, or when asked to run the app on the phone.
---

# Checking a change on the Pixel

The owner's phone is a **Pixel 9a**, connected over adb wireless debugging. It is their
personal phone: don't change system settings you can't restore, don't open other apps, and
put back anything you change (locale, dark mode, files you pushed).

## 1. Connect

```bash
adb devices
```

If the list is empty, pairing has expired. Re-pairing is a physical action: ask the owner in
the final message (and in STATUS "Needs the owner"; Remote Control and push don't reach them),
then go on with work that doesn't need the phone. Never ask for or store pairing codes.

## 2. Install the dev build

The debug build is `com.dibillsion.app.dev` ("DiBillSion dev") and lives side by side with the
testers' release app `com.dibillsion.app`. Never uninstall or overwrite the release app: its
local data is the owner's real groups.

```bash
cd android && ./gradlew :app:installDebug
adb shell am start -n com.dibillsion.app.dev/com.dibillsion.app.MainActivity
```

The dev build syncs with `dibill.debugSyncUrl` from `~/.gradle/gradle.properties` if set,
otherwise `http://127.0.0.1:8080` through `adb reverse tcp:8080 tcp:8080` with a local server
(`docs/STATUS.md`, "Dev server").

## 3. Look at it

```bash
adb exec-out screencap -p > <scratchpad>/screen.png      # then open it with the Read tool
adb shell uiautomator dump /sdcard/ui.xml && adb exec-out cat /sdcard/ui.xml   # bounds for taps
adb shell input tap X Y
adb shell input text 'Pizza'          # spaces as %s
adb shell input keyevent KEYCODE_BACK
```

Save screenshots in the session scratchpad, never in the repo. Pressing BACK on the receipt
review with no keyboard open asks to discard the receipt.

Check each changed screen in:
- **Spanish**: `adb shell cmd locale set-app-locales com.dibillsion.app.dev --locales es-ES`
  (restart the app; reset with `--locales ""`). Don't change the phone's system language.
- **Dark mode**: `adb shell cmd uimode night yes`, then put back what `adb shell cmd uimode night`
  reported before.
- Long names and big amounts (overflow), an empty group, a foreign currency.

## 4. Logs

```bash
adb logcat -d --pid=$(adb shell pidof com.dibillsion.app.dev) | tail -200
```

## 5. Receipt scan without a real photo

Push an image from `eval/receipts/`, make it the newest so it sorts first in the photo picker
(adb push keeps the old mtime, which hides it among the owner's personal photos):

```bash
adb push eval/receipts/<file>.png /sdcard/Pictures/
adb shell touch /sdcard/Pictures/<file>.png
adb shell content call --uri content://media --method scan_volume --arg external_primary
```

Afterwards delete it (`adb shell rm /sdcard/Pictures/<file>.png`) and run the scan again.
Each scan uses one of the 20 daily scans on the server (Claude Haiku), so don't loop.

## 6. Report

Say in the PR's "How it was tested" what you checked on the phone and attach nothing binary to
the repo. If the phone wasn't available, say so: the change is then only unit-tested.
