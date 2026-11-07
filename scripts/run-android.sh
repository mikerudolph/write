#!/bin/sh
set -eu

. scripts/android-emulator.sh

$adb install -r apps/android/app/build/outputs/apk/debug/app-debug.apk
$adb shell am start -n com.mikerudolph.write/.MainActivity
