#!/usr/bin/env bash
# The rendering smoke of the APK on the device adb reaches (the CI emulator,
# or any emulator or tablet): install, launch, and assert that the face draws
# and animates.
#
#   smoke-test.sh <apk> <out-dir> [second-face.glb]
#
# Passes when, after the launch:
#   - a screenshot taken at least 15 s after launch, with the app in front,
#     shows the face: enough pixels away from the black clear colour, polled
#     every 5 s up to VIZIJ_SMOKE_DEADLINE seconds (lavapipe's first frame
#     lands about 10 s in);
#   - two screenshots 5 s apart differ in the face (its program animates it);
#     up to five pairs are taken, so a moment where the face rests in the same
#     pose twice does not fail the run;
#   - the app is alive and in front at the end;
#   - with a second face given, opened through a VIEW intent (a content://
#     URI of the file pushed to Downloads, as a file manager sends), it
#     replaces the face on screen, and after the app is stopped and launched
#     again it is the face shown (the activity's `model.glb`);
#   - the app's log has no `Caught` line (Bevy's report of a device loss or
#     a rendering error), no panic and no fatal signal.
# `dumpsys gfxinfo` counts HWUI frames only and reads 0 for Bevy, so the
# screenshots are the frame evidence. <out-dir> keeps the two screenshots
# (face-a.png, face-b.png), with a second face opened.png and
# relaunched.png, logcat.txt (the app's own lines in logcat-app.txt),
# meminfo.txt and checks.txt.
#
# The emulator-runner action runs its `script` line by line, so the whole
# smoke lives here.
set -uo pipefail

APK=$1
OUT=$2
SECOND=${3:-}
PKG=${VIZIJ_SMOKE_PACKAGE:-ai.vizij.app}
DEADLINE=${VIZIJ_SMOKE_DEADLINE:-120}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"
rm -f "$OUT"/*.png "$OUT"/*.raw "$OUT"/checks.txt

failures=()
fail() {
  echo "::error::$1"
  failures+=("$1")
}
frame() { adb exec-out screencap > "$OUT/$1.raw"; }
measure() { python3 "$HERE/frames.py" "$@"; }
resumed() { adb shell dumpsys activity activities | grep -m1 -E 'topResumedActivity|mResumedActivity' | tr -d '\r'; }
in_front() { case "$(resumed)" in *"$PKG/"*) return 0 ;; *) return 1 ;; esac; }

adb shell getprop ro.build.version.sdk | sed 's/^/sdk=/'
adb shell wm size
adb uninstall "$PKG" >/dev/null 2>&1 || true
adb install -r "$APK" || { fail "install failed"; exit 1; }
# The system's one-time "Viewing full screen" hint would cover the face.
adb shell settings put secure immersive_mode_confirmations confirmed
adb logcat -c
launcher=$(adb shell cmd package resolve-activity --brief -c android.intent.category.LAUNCHER "$PKG" | tr -d '\r' | tail -1)
echo "launching $launcher"
adb shell am start -W -n "$launcher"
launched=$(date +%s)

# The first capture at 15 s, then every 5 s until the face shows.
sleep 15
shown=""
while :; do
  elapsed=$(( $(date +%s) - launched ))
  frame first
  # A screen in front of the app (the launcher after a crash) is lit too.
  if in_front && measure face "$OUT/first.raw"; then
    shown=$elapsed
    break
  fi
  if [ "$elapsed" -ge "$DEADLINE" ]; then
    break
  fi
  sleep 5
done

if [ -z "$shown" ]; then
  fail "no face on screen ${DEADLINE} s after launch"
  measure png "$OUT/first.raw" "$OUT/face-a.png"
else
  echo "face on screen ${shown} s after launch"
  # Pairs 5 s apart until one differs in the face.
  moved=""
  prev="$OUT/first.raw"
  for i in 1 2 3 4 5; do
    sleep 5
    frame "next$i"
    if measure moved "$prev" "$OUT/next$i.raw"; then
      moved=1
      measure png "$prev" "$OUT/face-a.png"
      measure png "$OUT/next$i.raw" "$OUT/face-b.png"
      break
    fi
    prev="$OUT/next$i.raw"
  done
  if [ -z "$moved" ]; then
    fail "the face did not move between screenshots 5 s apart"
    measure png "$OUT/first.raw" "$OUT/face-a.png"
    measure png "$prev" "$OUT/face-b.png"
  fi
fi

# A second face, opened through a VIEW intent while the app runs, then
# shown again on the next launch.
if [ -n "$SECOND" ] && [ -n "$shown" ]; then
  adb push "$SECOND" /sdcard/Download/vizij-smoke.glb > /dev/null
  # The media store indexes the file in its own time: ask, and look, for up
  # to a minute.
  id=""
  for _ in $(seq 1 12); do
    adb shell am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE \
      -d file:///sdcard/Download/vizij-smoke.glb > /dev/null
    sleep 5
    id=$(adb shell content query --uri content://media/external/downloads --projection _id:_display_name \
      | tr -d '\r' | grep 'vizij-smoke.glb' | sed -E 's/.*_id=([0-9]+).*/\1/' | tail -1)
    [ -n "$id" ] && break
  done
  if [ -z "$id" ]; then
    adb shell ls -l /sdcard/Download
    adb shell content query --uri content://media/external/downloads --projection _id:_display_name:_data
    fail "the second face is not in Downloads"
  else
    frame before
    adb shell am start -a android.intent.action.VIEW -t model/gltf-binary --grant-read-uri-permission \
      -d "content://media/external/downloads/$id"
    sleep 20
    frame opened
    measure png "$OUT/opened.raw" "$OUT/opened.png"
    if ! { in_front && measure face "$OUT/opened.raw" && measure replaced "$OUT/before.raw" "$OUT/opened.raw"; }; then
      fail "the face opened through a VIEW intent is not shown"
    fi
    adb shell am force-stop "$PKG"
    adb shell am start -W -n "$launcher" > /dev/null
    sleep 25
    frame relaunched
    measure png "$OUT/relaunched.raw" "$OUT/relaunched.png"
    if ! { in_front && measure face "$OUT/relaunched.raw"; }; then
      fail "no face on screen after the relaunch"
    fi
    if ! adb logcat -d | grep -q "vizij: the face last opened"; then
      fail "the relaunch did not open the face last opened"
    fi
  fi
fi
rm -f "$OUT"/*.raw

pid=$(adb shell pidof "$PKG" | tr -d '\r')
front=$(resumed)
echo "pid=${pid:-none}; ${front}"
[ -n "$pid" ] || fail "the app is not running"
in_front || fail "the app is not in front"

adb shell dumpsys meminfo "$PKG" > "$OUT/meminfo.txt" 2>&1 || true
# The whole log, and the app's own lines, which the checks read: system
# processes log `Caught` too (`RkpdBootReceiver: Caught boot intent`).
adb logcat -d -v threadtime > "$OUT/logcat.txt"
uid=$(adb shell pm list packages -U "$PKG" | tr -d '\r' | sed -nE "s/^package:$PKG uid:([0-9]+).*/\1/p")
adb logcat -d -v threadtime --uid "$uid" > "$OUT/logcat-app.txt"
sed -n '/App Summary/,/TOTAL SWAP/p' "$OUT/meminfo.txt"
echo "--- adapter and errors in the app's log"
grep -E "AdapterInfo|Caught|panicked|Encountered a panic|Quitting the application|Fatal signal|FATAL EXCEPTION|UnsatisfiedLinkError|vizij:" "$OUT/logcat-app.txt" | cut -c1-240 | head -40
if [ ! -s "$OUT/logcat-app.txt" ]; then fail "no log from the app (uid ${uid:-unknown})"; fi
if grep -qE "Caught" "$OUT/logcat-app.txt"; then fail "the app's log has a Caught line"; fi
if grep -qE "panicked at|Encountered a panic" "$OUT/logcat-app.txt"; then fail "the app panicked"; fi
if grep -qE "Fatal signal|FATAL EXCEPTION|UnsatisfiedLinkError" "$OUT/logcat-app.txt"; then fail "the app crashed"; fi

{
  echo "face shown after: ${shown:-never} s"
  echo "pid: ${pid:-none}"
  echo "resumed: ${front}"
  printf 'failure: %s\n' ${failures[@]+"${failures[@]}"}
} > "$OUT/checks.txt"
if [ "${#failures[@]}" -gt 0 ]; then
  echo "smoke failed: ${failures[*]}"
  exit 1
fi
echo "smoke passed: the face draws and animates"
