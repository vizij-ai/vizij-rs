#!/bin/bash
# Usage: run-anim.sh <avd> <port> <label> <apk> [seconds]
# Boots <avd> headless with -gpu host, runs the timeline case with <apk>, kills the emulator.
set -u
ANDROID_HOME=${ANDROID_HOME:-$HOME/Library/Android/sdk}
ADB=$ANDROID_HOME/platform-tools/adb
AVD=$1; PORT=$2; LABEL=$3; APK=$4; SECS=${5:-60}
S=$(cd "$(dirname "$0")" && pwd); SERIAL=emulator-$PORT
$ADB -s $SERIAL emu kill >/dev/null 2>&1; sleep 5
nohup $ANDROID_HOME/emulator/emulator -avd "$AVD" -no-window -no-audio -no-snapshot -no-boot-anim -gpu host -port "$PORT" > "$S/emulator-$LABEL.log" 2>&1 &
n=0
until [ "$($ADB -s $SERIAL shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do
  sleep 5; n=$((n+5)); [ $n -gt 900 ] && { echo "$LABEL: boot timeout"; exit 1; }
done
echo "$LABEL: booted after ~${n}s"; sleep 25
"$S/run-timeline.sh" "$LABEL" "$APK" "$SERIAL" "$SECS"
$ADB -s $SERIAL emu kill >/dev/null 2>&1; sleep 5
echo "$LABEL: done"
