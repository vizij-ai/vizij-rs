#!/bin/bash
# Usage: run-sw-image.sh <avd> <port> <gpu-mode> <label> <apk> [seconds]
# Boots <avd> headless with -gpu <gpu-mode>, runs the Vulkan probe and the
# timeline case with <apk>, kills the emulator.
set -u
ANDROID_HOME=${ANDROID_HOME:-$HOME/Library/Android/sdk}
ADB=$ANDROID_HOME/platform-tools/adb
AVD=$1; PORT=$2; GPU=$3; LABEL=$4; APK=$5; SECS=${6:-60}
S=$(cd "$(dirname "$0")" && pwd); SERIAL=emulator-$PORT
$ADB -s $SERIAL emu kill >/dev/null 2>&1; sleep 5
nohup $ANDROID_HOME/emulator/emulator -avd "$AVD" -no-window -no-audio -no-snapshot -no-boot-anim -gpu "$GPU" -port "$PORT" > "$S/emulator-$LABEL.log" 2>&1 &
n=0
until [ "$($ADB -s $SERIAL shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do
  sleep 5; n=$((n+5)); [ $n -gt 900 ] && { echo "$LABEL: boot timeout"; $ADB -s $SERIAL emu kill >/dev/null 2>&1; exit 1; }
done
echo "$LABEL: booted after ~${n}s"; sleep 25
grep -E "GPU Renderer|Selecting Vulkan device|vulkan_mode_selected" "$S/emulator-$LABEL.log" | cut -c1-200
$ADB -s $SERIAL shell wm size
$ADB -s $SERIAL push "$S/target-vkprobe/aarch64-linux-android/release/vkprobe" /data/local/tmp/vkprobe >/dev/null
$ADB -s $SERIAL shell chmod 755 /data/local/tmp/vkprobe
$ADB -s $SERIAL shell /data/local/tmp/vkprobe > "$S/vkprobe-$LABEL.txt" 2>&1
grep -E "device \"|maintenance4.max|max_buffer_size" "$S/vkprobe-$LABEL.txt"
"$S/run-timeline.sh" "$LABEL" "$APK" "$SERIAL" "$SECS"
grep -E "Qsri|VkDevice" "$S/emulator-$LABEL.log" | tail -4
$ADB -s $SERIAL emu kill >/dev/null 2>&1; sleep 5
echo "$LABEL: done"
