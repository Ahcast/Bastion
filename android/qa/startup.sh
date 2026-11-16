#!/usr/bin/env bash
set -euo pipefail
out=output/android-startup
mkdir -p "$out"
adb shell getprop ro.build.version.release > "$out/android-version.txt"
adb shell pm list packages > "$out/packages.txt"
adb install -r "$1"
adb shell am force-stop net.castilloit.bastion.preview
adb logcat -c
adb shell am start -W -n net.castilloit.bastion.preview/net.castilloit.bastion.MainActivity > "$out/start.txt"
sleep 8
adb logcat -d -s AndroidRuntime:E > "$out/crashes.txt"
adb logcat -d -s TWALauncherActivity:V TwaLauncher:V chromium:E > "$out/browser.txt"
adb shell dumpsys activity activities > "$out/activities.txt"
adb exec-out screencap -p > "$out/startup.png"
adb shell uiautomator dump /sdcard/bastion-startup.xml >/dev/null
adb pull /sdcard/bastion-startup.xml "$out/startup.xml" >/dev/null
cat "$out/start.txt"
if grep -q 'net.castilloit.bastion' "$out/crashes.txt"; then
  tail -60 "$out/crashes.txt"
  exit 1
fi
grep -E 'mResumedActivity|topResumedActivity' "$out/activities.txt" > "$out/resumed.txt"
cat "$out/resumed.txt"
if ! grep -Eq 'com.android.chrome|org.chromium|net.castilloit.bastion' "$out/resumed.txt"; then
  echo 'No browser or Bastion screen became visible.'
  exit 1
fi
