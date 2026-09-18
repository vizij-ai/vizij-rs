#!/bin/bash
# Usage: run-host-image.sh <avd> <port> <label>
# Starts <avd> headless with -gpu host on <port> (killing whatever runs there),
# waits for boot (up to 15 min), runs the Vulkan probe, the Vulkan timeline case
# and the GLES case, then kills the emulator.
set -u
ANDROID_HOME=${ANDROID_HOME:-$HOME/Library/Android/sdk}
ADB=$ANDROID_HOME/platform-tools/adb
AVD=$1; PORT=$2; LABEL=$3
S=$(cd "$(dirname "$0")" && pwd); B=$S/../bevy-android
SERIAL=emulator-$PORT
$ADB -s $SERIAL emu kill >/dev/null 2>&1; sleep 5
nohup $ANDROID_HOME/emulator/emulator -avd "$AVD" -no-window -no-audio -no-snapshot -no-boot-anim -gpu host -port "$PORT" > "$S/emulator-$LABEL.log" 2>&1 &
n=0
until [ "$($ADB -s $SERIAL shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do
  sleep 5; n=$((n+5)); [ $n -gt 900 ] && { echo "$LABEL: boot timeout after ${n}s"; exit 1; }
done
echo "$LABEL: booted after ~${n}s"; sleep 20
grep -E "GPU Renderer|GPU Version|Selecting Vulkan device|vulkan_mode_selected" "$S/emulator-$LABEL.log" | cut -c1-200
$ADB -s $SERIAL push "$S/target-vkprobe/aarch64-linux-android/release/vkprobe" /data/local/tmp/vkprobe >/dev/null
$ADB -s $SERIAL shell chmod 755 /data/local/tmp/vkprobe
$ADB -s $SERIAL shell /data/local/tmp/vkprobe > "$S/vkprobe-$LABEL.txt" 2>&1
grep -E "device \"|maintenance4.max|max_buffer_size|mutable" "$S/vkprobe-$LABEL.txt"
"$S/run-timeline.sh" "$LABEL-vulkan" "$B/bevy-spike-vulkan-debug.apk" "$SERIAL" 45
"$S/run-case2.sh" "$LABEL-gles" "$B/bevy-spike-gles-debug.apk" "$SERIAL"
$ADB -s $SERIAL emu kill >/dev/null 2>&1; sleep 5
echo "$LABEL: done"
