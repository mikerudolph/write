#!/bin/sh
set -eu

. scripts/android-emulator.sh

package=com.mikerudolph.write
screen=build/android-smoke-screen.xml
failures=0

require_foreground() {
  if ! $adb shell dumpsys activity activities | grep 'topResumedActivity' | grep -q "$package"; then
    echo "FAILURE: A different application is in the foreground"
    exit 1
  fi
}

read_screen() {
  sleep 1
  $adb shell uiautomator dump /sdcard/window.xml > /dev/null
  $adb shell cat /sdcard/window.xml > "$screen"
}

check() {
  if grep -q "$1" "$screen"; then
    echo "PASS: $2"
  else
    echo "FAILURE: $2"
    failures=$((failures + 1))
  fi
}

tap_text() {
  read_screen
  bounds=$(tr '>' '\n' < "$screen" | grep "text=\"$1\"" | sed -n 's/.*bounds="\[\([0-9]*\),\([0-9]*\)\]\[\([0-9]*\),\([0-9]*\)\]".*/\1 \2 \3 \4/p' | head -1)
  if [ -z "$bounds" ]; then
    echo "FAILURE: The screen does not show $1"
    failures=$((failures + 1))
    return
  fi
  set -- $bounds
  require_foreground
  $adb shell input tap $((($1 + $3) / 2)) $((($2 + $4) / 2))
}

launch() {
  $adb shell input keyevent KEYCODE_HOME
  sleep 2
  $adb shell am start -n "$package/.MainActivity" > /dev/null
  sleep 3
  require_foreground
}

return_to_list() {
  for attempt in 1 2 3; do
    read_screen
    grep -q 'text="New document"' "$screen" && return
    require_foreground
    $adb shell input keyevent KEYCODE_BACK
  done
  read_screen
}

mkdir -p build
$adb shell settings put secure stylus_handwriting_enabled 0
$adb install -r apps/android/app/build/outputs/apk/debug/app-debug.apk > /dev/null
$adb shell pm clear "$package" > /dev/null
launch

tap_text 'New document'
sleep 1
require_foreground
$adb shell input text 'Hello%sfrom%sWrite'
$adb shell input keyevent KEYCODE_ENTER
$adb shell input text 'second%sline'
read_screen
check 'Hello from Write' 'Key events put text in the editor'
check '5 words, 28 characters' 'The Rust store counts the text'

return_to_list
check 'text="Hello from Write"' 'The list shows the document title'

tap_text 'New document'
sleep 1
require_foreground
$adb shell input text 'Another%snote'
sleep 1
$adb shell am force-stop "$package"
launch
read_screen
check 'text="Hello from Write"' 'A new launch lists the first document'
check 'text="Another note"' 'A forced stop keeps the last document with no save action'

tap_text 'Hello from Write'
read_screen
check 'Hello from Write&#10;[Ss]econd line' 'A new launch restores the text'
check '5 words, 28 characters' 'A new launch gives the same count'

return_to_list
tap_text 'New document'
sleep 1
require_foreground
tap_text 'Math'
read_screen
check 'E = mc^2' 'The Math button opens the block editor with a starter'
$adb shell input text '+1'
sleep 1
read_screen
check 'E = mc^2+1' 'The block editor receives text'
$adb exec-out screencap -p > build/android-block-editor.png
tap_text '✕'
sleep 1
read_screen
check 'After\|class="android.widget.EditText"' 'The close control returns to the editor'
tap_text 'Table'
read_screen
check 'First' 'The Table button opens the block editor with a starter'
tap_text '✕'
tap_text 'Diagram'
read_screen
check 'flowchart LR' 'The Diagram button opens the block editor with a starter'
tap_text '✕'
sleep 1
$adb shell input text 'After%sblocks'
sleep 1
$adb exec-out screencap -p > build/android-blocks.png
read_screen
check 'After blocks' 'Typing continues after rich blocks'
check '2 words, 15 characters' 'A rich block is not a word or a character'
$adb shell am force-stop "$package"
launch
tap_text 'After blocks'
sleep 1
read_screen
editor=$(tr '>' '\n' < "$screen" | grep 'EditText' | sed -n 's/.*bounds="\[\([0-9]*\),\([0-9]*\)\]\[\([0-9]*\),\([0-9]*\)\]".*/\1 \2 \3 \4/p' | head -1)
set -- $editor
require_foreground
$adb shell input tap $(($1 + 90)) $(($2 + 60))
sleep 2
read_screen
check 'E = mc^2+1' 'A tap on a block after a new launch opens its source'
$adb exec-out screencap -p > build/android-block-tap.png
tap_text 'Delete block'
sleep 1
read_screen
if grep -q 'E = mc' "$screen"; then
  echo "FAILURE: Delete block closes the block"
  failures=$((failures + 1))
else
  echo "PASS: Delete block closes the block"
fi
check 'After blocks' 'Delete block keeps the other text'

require_foreground
tap_text '☰'
read_screen
check 'text="New document"' 'The menu icon opens the document list'

long_press_text() {
  read_screen
  bounds=$(tr '>' '\n' < "$screen" | grep "text=\"$1\"" | sed -n 's/.*bounds="\[\([0-9]*\),\([0-9]*\)\]\[\([0-9]*\),\([0-9]*\)\]".*/\1 \2 \3 \4/p' | head -1)
  set -- $bounds
  x=$((($1 + $3) / 2))
  y=$((($2 + $4) / 2))
  require_foreground
  $adb shell input swipe $x $y $x $y 900
}

return_to_list
long_press_text 'Another note'
sleep 1
read_screen
if grep -q 'text="Another note"' "$screen"; then
  echo "FAILURE: A long press moves a document to Trash"
  failures=$((failures + 1))
else
  echo "PASS: A long press moves a document to Trash"
fi
tap_text 'Trash'
read_screen
check 'text="Another note"' 'Trash shows the document inline'
check 'text="✕"' 'Trash has a close control'
tap_text 'Restore'
tap_text '✕'
read_screen
check 'text="Another note"' 'Restore returns the document to the list'

long_press_text 'Another note'
sleep 1
tap_text 'Trash'
tap_text 'Delete'
read_screen
check 'You cannot undo this' 'A permanent delete asks for confirmation in the row'
tap_text 'Delete forever'
sleep 1
$adb shell am force-stop "$package"
launch
read_screen
if grep -q 'Another note' "$screen"; then
  echo "FAILURE: A permanent delete stays after a new launch"
  failures=$((failures + 1))
else
  echo "PASS: A permanent delete stays after a new launch"
fi
check 'text="Hello from Write"' 'A permanent delete keeps other documents'

[ "$failures" -eq 0 ]
