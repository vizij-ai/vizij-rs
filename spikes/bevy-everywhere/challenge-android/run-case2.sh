#!/bin/bash
# Usage: run-case2.sh <case-name> <apk> <serial>
# Uninstalls, installs, launches the spike activity, waits 20 s, takes two
# screenshots 4 s apart, and records:
#   <case>.png, <case>-b.png, <case>-logcat.txt, <case>-gfxinfo.txt,
#   <case>-sf.txt (SurfaceFlinger layer latency), <case>-summary.txt
set -u
ANDROID_HOME=${ANDROID_HOME:-$HOME/Library/Android/sdk}
ADB="$ANDROID_HOME/platform-tools/adb -s $3"
CASE=$1; APK=$2
PKG=ai.vizij.bevyspike
OUT=$(cd "$(dirname "$0")" && pwd)
{
echo "case=$CASE apk=$(basename "$APK") serial=$3 date=$(date -u +%FT%TZ)"
$ADB shell getprop ro.build.version.release | sed 's/^/android=/'
$ADB shell getprop ro.build.version.sdk | sed 's/^/sdk=/'
$ADB shell wm size | sed 's/^/wm=/'
$ADB shell am force-stop $PKG
$ADB uninstall $PKG >/dev/null 2>&1
$ADB install "$APK" 2>&1 | tail -1 | sed 's/^/install=/'
$ADB logcat -c
$ADB shell am start -W -n $PKG/android.app.NativeActivity 2>&1 | grep -E "Status|TotalTime|Error" | tr '\n' ' '; echo
sleep 20
PID=$($ADB shell pidof $PKG | tr -d '\r')
echo "pid_after_20s=${PID:-none}"
$ADB shell dumpsys activity activities 2>/dev/null | grep -E "topResumedActivity|mResumedActivity" | head -1 | sed 's/^/resumed=/'
$ADB exec-out screencap -p > "$OUT/$CASE.png"
sleep 4
$ADB exec-out screencap -p > "$OUT/$CASE-b.png"
python3 - "$OUT/$CASE.png" "$OUT/$CASE-b.png" <<'PY'
import sys
from PIL import Image, ImageChops
def stats(p):
    im=Image.open(p).convert('RGB'); w,h=im.size
    sm=im.resize((w//8,h//8)); px=list(sm.getdata())
    orange=sum(1 for r,g,b in px if r>170 and 40<g<140 and b<110)
    clear=sum(1 for r,g,b in px if 8<=r<=24 and 10<=g<=26 and 16<=b<=32)
    return im, f"center {im.getpixel((w//2,h//2))} orange_cube_px {orange} clear_color_px {clear} of {len(px)}"
a,sa=stats(sys.argv[1]); b,sb=stats(sys.argv[2])
print("pixels_a:", sa); print("pixels_b:", sb)
diff=ImageChops.difference(a,b).convert('L'); bbox=diff.getbbox()
changed=sum(1 for v in diff.resize((a.width//8,a.height//8)).getdata() if v>24)
print("diff_a_b: bbox", bbox, "changed_px", changed)
PY
$ADB shell dumpsys gfxinfo $PKG 2>/dev/null > "$OUT/$CASE-gfxinfo.txt"
grep -E "Total frames rendered" "$OUT/$CASE-gfxinfo.txt" | sed 's/^/gfxinfo: /'
$ADB shell dumpsys SurfaceFlinger --list 2>/dev/null | grep -i bevyspike | head -3 | sed 's/^/sf_layer=/'
LAYER=$($ADB shell dumpsys SurfaceFlinger --list 2>/dev/null | grep -i "SurfaceView.*bevyspike\|bevyspike.*SurfaceView" | grep -v "#" | head -1 | tr -d '\r')
$ADB shell dumpsys SurfaceFlinger --latency "$LAYER" 2>/dev/null > "$OUT/$CASE-sf.txt"
echo "sf_latency_rows=$(grep -cvE '^(0|9223372036854775807)\s' "$OUT/$CASE-sf.txt")"
$ADB logcat -d > "$OUT/$CASE-logcat.txt"
echo "logcat_lines=$(wc -l < "$OUT/$CASE-logcat.txt")"
sed 's/\x1b\[[0-9;]*m//g' "$OUT/$CASE-logcat.txt" | grep -E "AdapterInfo|maximum buffer size|SURFACE_VIEW_FORMATS|panicked at|Unable to find a GPU|Caught|Surface is not configured|Encountered a panic|fit: bounds|GPU clustering|GPU preprocessing|Quitting|DeviceLost|Shader compilation|time_system|Creating new window|Fatal signal" | grep -vE " event" | cut -c1-260 | sed 's/^/log: /' | head -30
} 2>&1 | tee "$OUT/$CASE-summary.txt"
