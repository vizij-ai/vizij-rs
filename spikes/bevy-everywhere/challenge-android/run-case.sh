#!/bin/bash
# Usage: run-case.sh <case-name> <apk> [serial]
# Installs the APK, launches the spike activity, waits 10 s, and records:
#   <case>.png (screencap), <case>-logcat.txt, <case>-gfxinfo.txt, <case>-summary.txt
set -u
ANDROID_HOME=${ANDROID_HOME:-$HOME/Library/Android/sdk}
ADB=$ANDROID_HOME/platform-tools/adb
CASE=$1; APK=$2; SERIAL=${3:-}
[ -n "$SERIAL" ] && ADB="$ADB -s $SERIAL"
PKG=ai.vizij.bevyspike
OUT=$(dirname "$0")
{
echo "case=$CASE apk=$APK serial=$SERIAL date=$(date -u +%FT%TZ)"
$ADB shell getprop ro.build.version.release | sed 's/^/android=/'
$ADB shell getprop ro.build.version.sdk | sed 's/^/sdk=/'
$ADB shell getprop ro.hardware.vulkan | sed 's/^/ro.hardware.vulkan=/'
$ADB shell getprop ro.hardware.egl | sed 's/^/ro.hardware.egl=/'
$ADB shell wm size | sed 's/^/wm=/'
$ADB shell am force-stop $PKG
$ADB install -r "$APK" 2>&1 | tail -1 | sed 's/^/install=/'
$ADB logcat -c
$ADB shell am start -W -n $PKG/android.app.NativeActivity 2>&1 | grep -E "Status|TotalTime|Error" | tr '\n' ' '; echo
sleep 10
PID=$($ADB shell pidof $PKG | tr -d '\r')
echo "pid_after_10s=${PID:-none}"
$ADB shell dumpsys activity activities 2>/dev/null | grep -E "topResumedActivity|mResumedActivity" | head -2 | sed 's/^/resumed=/'
$ADB exec-out screencap -p > "$OUT/$CASE.png"
echo "screencap_bytes=$(stat -f %z "$OUT/$CASE.png")"
python3 -c "
from PIL import Image
im=Image.open('$OUT/$CASE.png').convert('RGB'); w,h=im.size
sm=im.resize((w//8,h//8)); px=list(sm.getdata())
orange=sum(1 for r,g,b in px if r>170 and 40<g<140 and b<110)
clear=sum(1 for r,g,b in px if 8<=r<=24 and 10<=g<=26 and 16<=b<=32)
print('pixels: center', im.getpixel((w//2,h//2)), 'orange_cube_px', orange, 'clear_color_px', clear, 'of', len(px))
" 2>&1 | tail -1
$ADB shell dumpsys gfxinfo $PKG 2>/dev/null > "$OUT/$CASE-gfxinfo.txt"
grep -E "Total frames rendered|Janky frames" "$OUT/$CASE-gfxinfo.txt" | sed 's/^/gfxinfo: /'
$ADB logcat -d > "$OUT/$CASE-logcat.txt"
echo "logcat_lines=$(wc -l < "$OUT/$CASE-logcat.txt")"
grep -E "AdapterInfo|maximum buffer size|SURFACE_VIEW_FORMATS|panicked at|Unable to find a GPU|Caught rendering error|Surface is not configured|Encountered a panic|fit: bounds|Failed to start Gilrs|GPU clustering" "$OUT/$CASE-logcat.txt" | grep -v "log event\|E event\|I event\|W event" | cut -c1-260 | sed 's/^/log: /' | head -30
} 2>&1 | tee "$OUT/$CASE-summary.txt"
