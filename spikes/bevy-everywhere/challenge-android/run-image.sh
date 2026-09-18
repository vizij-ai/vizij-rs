#!/bin/bash
# Usage: run-image.sh <avd> <port> <gpu-mode> <label> [vulkan] [gles]
# Restarts the emulator on <port> with -gpu <gpu-mode>, waits for boot, runs
# the Vulkan limits probe, then each requested APK case through run-case.sh.
set -u
ANDROID_HOME=${ANDROID_HOME:-$HOME/Library/Android/sdk}
ADB=$ANDROID_HOME/platform-tools/adb
AVD=$1; PORT=$2; GPU=$3; LABEL=$4; shift 4
S=$(cd "$(dirname "$0")" && pwd)
B=$S/../bevy-android
SERIAL=emulator-$PORT
$ADB -s $SERIAL emu kill >/dev/null 2>&1
for i in $(seq 1 30); do $ADB devices | grep -q "^$SERIAL" || break; sleep 2; done
pkill -f "emulator.*-port $PORT" 2>/dev/null; sleep 3
nohup $ANDROID_HOME/emulator/emulator -avd "$AVD" -no-window -no-audio -no-snapshot -no-boot-anim -gpu "$GPU" -port "$PORT" > "$S/emulator-$LABEL.log" 2>&1 &
n=0
until [ "$($ADB -s $SERIAL shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do
  sleep 5; n=$((n+5)); [ $n -gt 480 ] && { echo "$LABEL: boot timeout"; exit 1; }
done
echo "$LABEL: booted after ~${n}s"; sleep 15
grep -E "GPU Renderer|GPU Version|Selecting Vulkan device|vulkan_mode_selected" "$S/emulator-$LABEL.log" | cut -c1-200
$ADB -s $SERIAL push "$S/target-vkprobe/aarch64-linux-android/release/vkprobe" /data/local/tmp/vkprobe >/dev/null
$ADB -s $SERIAL shell chmod 755 /data/local/tmp/vkprobe
$ADB -s $SERIAL shell /data/local/tmp/vkprobe > "$S/vkprobe-$LABEL.txt" 2>&1
grep -E "device \"|maintenance|max_buffer_size" "$S/vkprobe-$LABEL.txt"
for c in "$@"; do
  case $c in
    vulkan) APK=$B/bevy-spike-vulkan-debug.apk ;;
    gles) APK=$B/bevy-spike-gles-debug.apk ;;
  esac
  "$S/run-case.sh" "$LABEL-$c" "$APK" "$SERIAL"
done
echo "$LABEL: done"
