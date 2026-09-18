#!/bin/bash
# Usage: run-live.sh <serial> <label> [vulkan] [gles]  -- on an already booted emulator
set -u
ANDROID_HOME=${ANDROID_HOME:-$HOME/Library/Android/sdk}
ADB="$ANDROID_HOME/platform-tools/adb -s $1"
S=$(cd "$(dirname "$0")" && pwd); B=$S/../bevy-android
SERIAL=$1; LABEL=$2; shift 2
$ADB push "$S/target-vkprobe/aarch64-linux-android/release/vkprobe" /data/local/tmp/vkprobe >/dev/null
$ADB shell chmod 755 /data/local/tmp/vkprobe
$ADB shell /data/local/tmp/vkprobe > "$S/vkprobe-$LABEL.txt" 2>&1
grep -E "device \"|maintenance|max_buffer_size" "$S/vkprobe-$LABEL.txt"
for c in "$@"; do
  case $c in
    vulkan) APK=$B/bevy-spike-vulkan-debug.apk ;;
    gles) APK=$B/bevy-spike-gles-debug.apk ;;
  esac
  "$S/run-case2.sh" "$LABEL-$c" "$APK" "$SERIAL"
done
echo "$LABEL: done"
