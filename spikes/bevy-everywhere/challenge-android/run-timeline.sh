#!/bin/bash
# Usage: run-timeline.sh <case-name> <apk> <serial> [seconds=45]
# Uninstall/install/launch, then a screenshot every 5 s with pixel stats, then logcat.
set -u
ANDROID_HOME=${ANDROID_HOME:-$HOME/Library/Android/sdk}
ADB="$ANDROID_HOME/platform-tools/adb -s $3"
CASE=$1; APK=$2; SECS=${4:-45}
PKG=ai.vizij.bevyspike
OUT=$(cd "$(dirname "$0")" && pwd)
{
echo "case=$CASE apk=$(basename "$APK") serial=$3 date=$(date -u +%FT%TZ)"
$ADB shell getprop ro.build.version.sdk | sed 's/^/sdk=/'
$ADB shell am force-stop $PKG; $ADB uninstall $PKG >/dev/null 2>&1
$ADB install "$APK" 2>&1 | tail -1 | sed 's/^/install=/'
$ADB logcat -c
$ADB shell am start -W -n $PKG/android.app.NativeActivity 2>&1 | grep -E "Status|TotalTime" | tr '\n' ' '; echo
T0=$(date +%s)
for t in $(seq 5 5 $SECS); do
  sleep 5
  F="$OUT/$CASE-t$t.png"
  $ADB exec-out screencap -p > "$F"
  python3 - "$F" "$t" <<'PY'
import sys
from PIL import Image
from collections import Counter
im=Image.open(sys.argv[1]).convert('RGB'); w,h=im.size
c=Counter(im.getpixel((x,y)) for y in range(120,h-300,16) for x in range(60,w-60,16))
top=c.most_common(2)
sm=im.resize((w//8,h//8)); px=list(sm.getdata())
orange=sum(1 for r,g,b in px if r>170 and 40<g<140 and b<110)
print(f"t={sys.argv[2]}s dominant={top} orange_px={orange} center={im.getpixel((w//2,h//2))}")
PY
done
PID=$($ADB shell pidof $PKG | tr -d '\r'); echo "pid_at_end=${PID:-none}"
LAYER=$($ADB shell dumpsys SurfaceFlinger --list 2>/dev/null | grep -E "^[0-9a-f]+ ai.vizij.bevyspike/android.app.NativeActivity#" | head -1 | tr -d '\r')
echo "sf_layer=$LAYER"
$ADB shell dumpsys SurfaceFlinger --latency "$LAYER" 2>/dev/null > "$OUT/$CASE-sf1.txt"; sleep 3
$ADB shell dumpsys SurfaceFlinger --latency "$LAYER" 2>/dev/null > "$OUT/$CASE-sf2.txt"
for f in sf1 sf2; do echo "$f: rows=$(grep -cvE '^(0|9223372036854775807)\s' "$OUT/$CASE-$f.txt") last_present_ns=$(awk 'NF==3 && $2!=0 && $2!="9223372036854775807"{l=$2} END{print l}' "$OUT/$CASE-$f.txt")"; done
$ADB shell dumpsys activity activities 2>/dev/null | grep -E "topResumedActivity" | head -1 | sed 's/^/resumed=/'
$ADB logcat -d > "$OUT/$CASE-logcat.txt"
sed 's/\x1b\[[0-9;]*m//g' "$OUT/$CASE-logcat.txt" | grep -E "AdapterInfo|maximum buffer size|SURFACE_VIEW_FORMATS|panicked at|Caught|Surface is not configured|fit: bounds|Quitting|DeviceLost|Shader compilation|time_system|Creating new window|Fatal signal|ANR in" | grep -vE " event" | cut -c1-200 | sed 's/^/log: /' | head -24
} 2>&1 | tee "$OUT/$CASE-summary.txt"
