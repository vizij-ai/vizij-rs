#!/bin/bash
# Usage: run-timeline2.sh <case-name> <apk> <serial> [seconds=60]
# Uninstall/install/launch, then a screenshot every 5 s. For each screenshot:
# dominant colours of the app area, orange-cube pixel count, and the number of
# changed pixels in the lower-left quadrant versus the previous screenshot (the
# spinning cube lives there). Then SurfaceFlinger layer names and latency,
# logcat, and the time from window creation to the `fit: bounds` line (180
# Update frames).
set -u
ANDROID_HOME=${ANDROID_HOME:-$HOME/Library/Android/sdk}
ADB="$ANDROID_HOME/platform-tools/adb -s $3"
CASE=$1; APK=$2; SECS=${4:-60}
PKG=ai.vizij.bevyspike
OUT=$(cd "$(dirname "$0")" && pwd)
{
echo "case=$CASE apk=$(basename "$APK") serial=$3 date=$(date -u +%FT%TZ)"
$ADB shell getprop ro.build.version.sdk | sed 's/^/sdk=/'
$ADB shell wm size | sed 's/^/wm=/'
$ADB shell am force-stop $PKG; $ADB uninstall $PKG >/dev/null 2>&1
$ADB install "$APK" 2>&1 | tail -1 | sed 's/^/install=/'
$ADB logcat -c
$ADB shell am start -W -n $PKG/android.app.NativeActivity 2>&1 | grep -E "Status|TotalTime" | tr '\n' ' '; echo
PREV=""
for t in $(seq 5 5 $SECS); do
  sleep 5
  F="$OUT/$CASE-t$t.png"
  $ADB exec-out screencap -p > "$F"
  python3 - "$F" "$t" "$PREV" <<'PY'
import sys
from PIL import Image, ImageChops
from collections import Counter
im=Image.open(sys.argv[1]).convert('RGB'); w,h=im.size
c=Counter(im.getpixel((x,y)) for y in range(120,h-300,16) for x in range(60,w-60,16))
top=c.most_common(2)
sm=im.resize((w//8,h//8)); px=list(sm.getdata())
orange=sum(1 for r,g,b in px if r>170 and 40<g<140 and b<110)
ll=""
if sys.argv[3]:
    prev=Image.open(sys.argv[3]).convert('RGB')
    box=(0,h//2,w//2,h-140)  # lower-left quadrant above the taskbar
    d=ImageChops.difference(im.crop(box),prev.crop(box)).convert('L')
    changed=sum(1 for v in d.getdata() if v>24)
    ll=f" lowerleft_changed_px={changed} of {(box[2]-box[0])*(box[3]-box[1])}"
print(f"t={sys.argv[2]}s dominant={top} orange_px={orange} center={im.getpixel((w//2,h//2))}{ll}")
PY
  PREV=$F
done
PID=$($ADB shell pidof $PKG | tr -d '\r'); echo "pid_at_end=${PID:-none}"
$ADB shell dumpsys SurfaceFlinger --list 2>/dev/null | grep -i bevyspike | tr -d '\r' | sed 's/^/sf_list: /'
$ADB shell dumpsys SurfaceFlinger --list 2>/dev/null | grep -i bevyspike | tr -d '\r' | while read -r LAYER; do
  $ADB shell dumpsys SurfaceFlinger --latency "$LAYER" 2>/dev/null > "$OUT/$CASE-sf.txt"
  ROWS=$(grep -cvE '^(0|9223372036854775807)\s' "$OUT/$CASE-sf.txt")
  echo "sf_latency[$LAYER]: rows=$ROWS"
  [ "$ROWS" -gt 1 ] && { awk 'NF==3 && $2!=0 && $2!="9223372036854775807"{print $2}' "$OUT/$CASE-sf.txt" | head -3 | tr '\n' ' '; echo " ... last present ts (ns): $(awk 'NF==3 && $2!=0 && $2!="9223372036854775807"{l=$2} END{print l}' "$OUT/$CASE-sf.txt")"; cp "$OUT/$CASE-sf.txt" "$OUT/$CASE-sf-best.txt"; }
done
$ADB shell dumpsys activity activities 2>/dev/null | grep -E "topResumedActivity" | head -1 | sed 's/^/resumed=/'
$ADB logcat -d > "$OUT/$CASE-logcat.txt"
sed 's/\x1b\[[0-9;]*m//g' "$OUT/$CASE-logcat.txt" | grep -E "AdapterInfo|maximum buffer size|SURFACE_VIEW_FORMATS|panicked at|Caught|Surface is not configured|fit: bounds|Quitting|DeviceLost|Shader compilation|time_system|Creating new window|Fatal signal|ANR in" | grep -vE " event" | cut -c1-200 | sed 's/^/log: /' | head -24
} 2>&1 | tee "$OUT/$CASE-summary.txt"
