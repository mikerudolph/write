#!/bin/sh
set -eu

. scripts/android-emulator.sh

package=com.mikerudolph.write
shared=/sdcard/Documents/WriteSync
work=build/e2e-$(date +%s)
cloud="$work/cloud/Write Library"
database="$work/mac.sqlite3"
screen="$work/screen.xml"
peer=target/release/examples/sync_peer
failures=0

mkdir -p "$cloud"
cargo build --release -q -p write-store --example sync_peer

read_screen() {
  sleep 1
  $adb shell uiautomator dump /sdcard/window.xml > /dev/null
  $adb shell cat /sdcard/window.xml > "$screen"
}

check_screen() {
  if grep -q "$1" "$screen"; then
    echo "PASS: $2"
  else
    echo "FAILURE: $2"
    failures=$((failures + 1))
  fi
}

check_text() {
  if printf '%s\n' "$1" | grep -q "$2"; then
    echo "PASS: $3"
  else
    echo "FAILURE: $3: $1"
    failures=$((failures + 1))
  fi
}

tap_text() {
  read_screen
  bounds=$(tr '>' '\n' < "$screen" | grep "text=\"$1\"" | sed -n 's/.*bounds="\[\([0-9]*\),\([0-9]*\)\]\[\([0-9]*\),\([0-9]*\)\]".*/\1 \2 \3 \4/p' | head -1)
  if [ -z "$bounds" ]; then
    echo "FAILURE: The screen does not show $1"
    exit 1
  fi
  set -- $bounds
  $adb shell input tap $((($1 + $3) / 2)) $((($2 + $4) / 2))
  sleep 2
}

relay_to_android() {
  $adb shell mkdir -p "'$shared/Write Library'"
  $adb push "$cloud/." "$shared/Write Library/" > /dev/null
}

relay_to_mac() {
  $adb pull "$shared/Write Library/." "$cloud/" > /dev/null
}

$adb shell settings put secure stylus_handwriting_enabled 0
$adb logcat -b crash -c
$adb shell rm -rf "$shared"
$adb shell mkdir -p "$shared"
$adb install -r apps/android/app/build/outputs/apk/debug/app-debug.apk > /dev/null
$adb shell pm clear "$package" > /dev/null
$adb shell am start -W -n "$package/.MainActivity" > /dev/null

tap_text 'Storage location'
tap_text 'Documents'
tap_text 'WriteSync'
tap_text 'USE THIS FOLDER'
tap_text 'ALLOW'
sleep 2

"$peer" "$database" "$cloud" add 'From the Mac'
relay_to_android
tap_text 'Sync now'
sleep 3
read_screen
check_screen 'text="From the Mac"' 'Android receives a document from the Mac'

tap_text 'From the Mac'
read_screen
check_screen 'From the Mac' 'Android opens the document text'

$adb shell input keyevent KEYCODE_MOVE_END
$adb shell input text '%sand%sAndroid'
sleep 5
$adb shell input keyevent KEYCODE_BACK
sleep 1
read_screen
grep -q 'text="New document"' "$screen" || $adb shell input keyevent KEYCODE_BACK
tap_text 'Sync now'
sleep 3
relay_to_mac
output=$("$peer" "$database" "$cloud" show)
check_text "$output" 'From the Mac and Android' 'The Mac receives the Android edit'

"$peer" "$database" "$cloud" append ' plus Mac'
tap_text 'From the Mac and Android'
sleep 1
$adb shell input keyevent KEYCODE_MOVE_HOME
$adb shell input text 'Phone%s'
sleep 5
relay_to_android
$adb shell input keyevent KEYCODE_BACK
sleep 1
read_screen
grep -q 'text="New document"' "$screen" || $adb shell input keyevent KEYCODE_BACK
tap_text 'Sync now'
sleep 3
relay_to_mac
output=$("$peer" "$database" "$cloud" show)
"$peer" "$database" "$cloud" show > /dev/null
relay_to_android
tap_text 'Sync now'
sleep 3
check_text "$output" 'Phone From the Mac and Android plus Mac' 'The Mac merges concurrent edits'
read_screen
check_screen 'text="Phone From the Mac and Android plus Mac"' 'Android merges concurrent edits'

fresh="$work/new-device.sqlite3"
output=$("$peer" "$fresh" "$cloud" show)
check_text "$output" 'Phone From the Mac and Android plus Mac' 'A new device rebuilds from the folder'

tap_editor_top() {
  read_screen
  editor=$(tr '>' '\n' < "$screen" | grep 'EditText' | sed -n 's/.*bounds="\[\([0-9]*\),\([0-9]*\)\]\[\([0-9]*\),\([0-9]*\)\]".*/\1 \2 \3 \4/p' | head -1)
  set -- $editor
  $adb shell input tap $(($1 + 90)) $(($2 + 60))
  sleep 2
  read_screen
}

leave_editor() {
  $adb shell input keyevent KEYCODE_BACK
  sleep 1
  read_screen
  grep -q 'text="New document"' "$screen" || $adb shell input keyevent KEYCODE_BACK
  sleep 1
}

$adb exec-out screencap -p > "$work/image.png"
"$peer" "$database" "$cloud" add-math 'x^2' > /dev/null
"$peer" "$database" "$cloud" add-image "$work/image.png" > /dev/null
relay_to_android
tap_text 'Sync now'
sleep 3
tap_text 'Math from the Mac'
tap_editor_top
check_screen 'x^2' 'Android opens a math block from the Mac'
$adb shell input text '+y'
sleep 1
tap_text '✕'
sleep 1
$adb exec-out screencap -p > "$work/android-math.png"
leave_editor
tap_text 'Image from the Mac'
tap_editor_top
check_screen 'Delete block' 'Android opens an image block from the Mac'
if grep -q 'not on this device' "$screen"; then
  echo "FAILURE: Android has the image bytes"
  failures=$((failures + 1))
else
  echo "PASS: Android has the image bytes"
fi
$adb exec-out screencap -p > "$work/android-image.png"
tap_text '✕'
sleep 1
leave_editor
tap_text 'Sync now'
sleep 3
relay_to_mac
output=$("$peer" "$database" "$cloud" show)
check_text "$output" 'embed math x^2+y' 'The Mac receives a block edit from Android'

"$peer" "$database" "$cloud" trash 'Image from the Mac' > /dev/null
relay_to_android
tap_text 'Sync now'
sleep 3
read_screen
if grep -q 'text="Image from the Mac"' "$screen"; then
  echo "FAILURE: Android receives Trash from the Mac"
  failures=$((failures + 1))
else
  echo "PASS: Android receives Trash from the Mac"
fi
tap_text 'Trash'
read_screen
check_screen 'text="Image from the Mac"' 'Android Trash shows the document'
tap_text 'Delete'
tap_text 'Delete forever'
sleep 3
relay_to_mac
output=$("$peer" "$database" "$cloud" trash-list)
if printf '%s\n' "$output" | grep -q 'trash Image from the Mac'; then
  echo "FAILURE: The Mac receives a permanent delete from Android"
  failures=$((failures + 1))
else
  echo "PASS: The Mac receives a permanent delete from Android"
fi
echo "Screens: $work"

$adb logcat -d -b crash > "$work/crash.log"
[ -s "$work/crash.log" ] && echo "FAILURE: Android logged a crash" && failures=$((failures + 1))

[ "$failures" -eq 0 ]
