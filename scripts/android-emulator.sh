tools="${ANDROID_HOME:-$HOME/Library/Android/sdk}/platform-tools"
serial=$("$tools/adb" devices | awk '/^emulator-[0-9]+[[:space:]]+device$/ { print $1; exit }')
if [ -z "$serial" ]; then
  echo "FAILURE: No running emulator" >&2
  exit 1
fi
adb="$tools/adb -s $serial"
