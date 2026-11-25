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
#   - a `say` invoked through the open local bridge (`adb forward`,
#     scripts/say.py) succeeds, the face reporting the utterance and showing
#     visemes while it plays: the cloud provider reached over TLS, its audio
#     played through AAudio;
#   - a tap on the face shows the settings pane's Settings button, which
#     opens the pane; a background picked there (a swatch) fills the screen
#     once the back gesture closes the pane, the app still in front, and
#     again after the app is stopped and launched again. The buttons are
#     found on the screenshots by their colours (scripts/frames.py find);
#   - the pane, opened again and dragged from a swatch to its Device
#     section at its end, stays over the face. Enroll is never pressed: the
#     app, holding no Studio credential, contacts nobody, and its log shows
#     no enrollment under way;
#   - the app is alive and in front at the end;
#   - with a second face given, opened through a VIEW intent (a content://
#     URI of the file pushed to Downloads, as a file manager sends), it
#     replaces the face on screen, and after the app is stopped and launched
#     again it is the face shown (the activity's `model.glb`);
#   - the app's log has no `Caught` line (Bevy's report of a device loss or
#     a rendering error), no panic and no fatal signal.
# adb loses the emulator now and then (`adb: device offline`) and finds it
# again within seconds: every adb call first waits for the device, and a
# screenshot cut short in that gap is taken again, so the checks read whole
# frames. The `say` waits for the emulator's network to be validated, since
# its provider is a cloud service; the provider's own error lines (`tts:`)
# are printed with the app's log.
# `dumpsys gfxinfo` counts HWUI frames only and reads 0 for Bevy, so the
# screenshots are the frame evidence. <out-dir> keeps the two screenshots
# (face-a.png, face-b.png), with a second face opened.png and
# relaunched.png, logcat.txt (the app's own lines in logcat-app.txt),
# meminfo.txt, say.txt and checks.txt; the pane's controls.png, pane.png,
# background.png, background-relaunched.png and device.png; and, when a
# system dialog came up over the app, the first ones seen (dialog-1.png...).
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
rm -f "$OUT"/*.png "$OUT"/*.raw "$OUT"/*.covered "$OUT"/checks.txt

failures=()
# A check that fails while a system dialog still covers the app has seen
# the dialog, not the app: it is reported as the dialog, naming the check.
fail() {
  local dialog=${covered:-$(system_dialog)}
  local message=$1
  [ -z "$dialog" ] || message="a system dialog covered the app (${dialog}); unchecked: $1"
  echo "::error::$message"
  failures+=("$message")
}
# Waits up to a minute for the device; a command it still cannot reach
# fails as before.
adb() {
  timeout 60 adb wait-for-device
  command adb "$@"
}
# A check on a frame taken with a system dialog over the app would measure
# the dialog (a light dialog over black passes `face`, and against a frame of
# the app passes `moved` and `replaced`), so it is not made: the check fails,
# and `covered` names the dialog for its report.
measure() {
  local arg
  case $1 in
    face | moved | replaced | find | background)
      for arg in "${@:2}"; do
        if [ -f "${arg%.raw}.covered" ]; then
          covered=$(cat "${arg%.raw}.covered")
          echo "${arg}: taken with a system dialog over the app, not measured"
          return 1
        fi
      done
      ;;
  esac
  python3 "$HERE/frames.py" "$@"
}

# A system dialog ("<app> isn't responding", "<app> keeps stopping") can
# come up over the app at any moment: on the runner's two cores the
# emulator's own launcher is slow enough to be reported as not responding.
# A screenshot then shows the dialog and a tap lands on it, so every
# screenshot and every input first clears it, pressing its Wait button
# (Close app on a crash dialog) or, with no button found, the back key,
# until no such window is left. One still there after DIALOG_DEADLINE
# seconds is left, and `covered` names it until a later clear succeeds; an
# input is then not sent, since it would land on the dialog (the centre of
# the screen is its Close app row), and the frame is marked as covered.
DIALOG_DEADLINE=${VIZIJ_SMOKE_DIALOG_DEADLINE:-60}
covered=""
dialogs=0
# The dialog's window title, as the window manager lists it
# ("Application Not Responding: <process>"), or nothing.
system_dialog() {
  adb shell dumpsys window windows | tr -d '\r' \
    | sed -nE 's/.*Window\{[0-9a-f]+ u[0-9]+ ((Application Not Responding|Application Error): [^}]*)\}.*/\1/p' \
    | head -1
}
DIALOG_BUTTON=$(cat <<'PY'
import re, sys
xml = sys.stdin.read()
for name in ("aerr_wait", "aerr_close"):
    m = re.search(r'resource-id="android:id/' + name + r'"[^>]*bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"', xml)
    if m:
        x1, y1, x2, y2 = map(int, m.groups())
        print((x1 + x2) // 2, (y1 + y2) // 2)
        break
PY
)
# The centre of the dialog's Wait (or Close app) button, from the window
# hierarchy; nothing when the dump fails, as it can while the system is busy.
dialog_button() {
  timeout 30 adb shell uiautomator dump /sdcard/window.xml > /dev/null 2>&1 || return 0
  adb exec-out cat /sdcard/window.xml | python3 -I -c "$DIALOG_BUTTON"
}
clear_dialogs() {
  local title at deadline=$(( $(date +%s) + DIALOG_DEADLINE ))
  while title=$(system_dialog); [ -n "$title" ]; do
    if [ "$(date +%s)" -ge "$deadline" ]; then
      echo "a system dialog still covers the app after ${DIALOG_DEADLINE} s: ${title}"
      covered=$title
      return 1
    fi
    dialogs=$((dialogs + 1))
    echo "a system dialog covers the app: ${title}"
    # The first few are kept, the evidence of what covered the app.
    if [ "$dialogs" -le 3 ]; then
      adb exec-out screencap > "$OUT/dialog-$dialogs.raw"
      measure png "$OUT/dialog-$dialogs.raw" "$OUT/dialog-$dialogs.png" 2> /dev/null || true
    fi
    at=$(dialog_button)
    # Looked for again just before pressing: a dialog gone in the meantime
    # leaves the app under the tap or the back key.
    [ -n "$(system_dialog)" ] || break
    if [ -n "$at" ]; then
      echo "pressing its button at ${at}"
      # shellcheck disable=SC2086 # "x y", two arguments
      adb shell input tap $at
    else
      echo "no button found; pressing back"
      adb shell input keyevent KEYCODE_BACK
    fi
    sleep 2
  done
  covered=""
}
input() {
  if ! clear_dialogs; then
    echo "not sent, a system dialog covers the app: input $*"
    return 1
  fi
  adb shell input "$@"
}
# A frame of the app: whole, and with no system dialog over it. A dialog
# that comes up as the frame is taken is cleared and the frame taken again;
# a frame a dialog still covers is marked (<name>.covered, the dialog's
# title), which `measure` refuses, and `frame` fails.
frame() {
  rm -f "$OUT/$1.covered"
  for _ in 1 2 3; do
    clear_dialogs
    adb exec-out screencap > "$OUT/$1.raw"
    if ! measure whole "$OUT/$1.raw"; then
      echo "$1: a partial screenshot, taken again"
    elif [ -n "$covered" ]; then
      break
    elif [ -n "$(system_dialog)" ]; then
      echo "$1: a system dialog came up as it was taken, taken again"
    else
      return 0
    fi
    sleep 2
  done
  [ -n "$covered" ] || covered=$(system_dialog)
  [ -z "$covered" ] && return 0
  echo "$covered" > "$OUT/$1.covered"
  echo "$1: taken with a system dialog over the app"
  return 1
}
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

# A `say` through the local bridge, followed to its end.
if [ -n "$shown" ]; then
  validated=""
  for _ in $(seq 1 12); do
    if adb shell dumpsys connectivity | grep -qw VALIDATED; then
      validated=1
      break
    fi
    sleep 5
  done
  [ -n "$validated" ] || echo "the emulator's network is not validated; saying anyway"
  adb forward tcp:9000 tcp:9000 > /dev/null
  python3 -I "$HERE/say.py" 9000 "Hello from Vizij on Android." | tee "$OUT/say.txt" \
    || fail "say did not speak"
  adb forward --remove tcp:9000
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
# The settings pane, opened and closed, and the background it keeps.
BUTTON=2563eb
SWATCH=0f766e
if [ -n "$shown" ]; then
  read -r width height < <(adb shell wm size | tr -d '\r' | sed -nE 's/.*: ([0-9]+)x([0-9]+).*/\1 \2/p' | tail -1)
  input tap $((width / 2)) $((height / 2))
  sleep 2
  frame controls
  measure png "$OUT/controls.raw" "$OUT/controls.png"
  if ! at=$(measure find "$OUT/controls.raw" "$BUTTON"); then
    fail "a tap on the face does not show the Settings button"
  else
    # shellcheck disable=SC2086 # "x y", two arguments
    input tap $at
    sleep 2
    frame pane
    measure png "$OUT/pane.raw" "$OUT/pane.png"
    if ! at=$(measure find "$OUT/pane.raw" "$SWATCH"); then
      fail "the Settings button does not open the pane"
    else
      # shellcheck disable=SC2086
      input tap $at
      sleep 1
      input keyevent KEYCODE_BACK
      sleep 2
      frame closed
      measure png "$OUT/closed.raw" "$OUT/background.png"
      in_front || fail "the back gesture with the pane open left the app"
      # The pane covers the screen: the background shows once it is closed.
      measure background "$OUT/closed.raw" "$SWATCH" \
        || fail "the background picked in the pane does not show once the pane is closed"
      adb shell am force-stop "$PKG"
      adb shell am start -W -n "$launcher" > /dev/null
      sleep 25
      frame kept
      measure png "$OUT/kept.raw" "$OUT/background-relaunched.png"
      { in_front && measure background "$OUT/kept.raw" "$SWATCH"; } \
        || fail "the background picked in the pane does not come back after a relaunch"
      # The Device section, at the end of the pane. The drag starts on the
      # swatch picked, a button, where no text takes it, and is slow enough
      # not to fling. Nothing is pressed in the section.
      input tap $((width / 2)) $((height / 2))
      sleep 2
      frame controls-again
      if ! at=$(measure find "$OUT/controls-again.raw" "$BUTTON"); then
        fail "a tap on the relaunched face does not show the Settings button"
      else
        # shellcheck disable=SC2086
        input tap $at
        sleep 2
        frame pane-again
        if ! swatch=$(measure find "$OUT/pane-again.raw" "$SWATCH"); then
          fail "the Settings button does not open the pane again"
        else
          read -r x y <<< "$swatch"
          to=$(( y - height * 45 / 100 ))
          [ "$to" -ge 5 ] || to=5
          input swipe "$x" "$y" "$x" "$to" 1000
          sleep 2
          frame device
          measure png "$OUT/device.raw" "$OUT/device.png"
          in_front || fail "the pane's Device section left the app"
          # The pane stays over the face, which the background picked does
          # not fill.
          if measure background "$OUT/device.raw" "$SWATCH"; then
            fail "the pane does not stay open on its Device section"
          fi
          input keyevent KEYCODE_BACK
          sleep 2
        fi
      fi
    fi
  fi
fi
rm -f "$OUT"/*.raw "$OUT"/*.covered

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
echo "--- applications the system found not responding"
grep -E "ActivityManager: (ANR in|Killing .*user request after error)" "$OUT/logcat.txt" | cut -c1-240 | head -10
echo "--- adapter and errors in the app's log"
grep -E "AdapterInfo|Caught|panicked|Encountered a panic|Quitting the application|Fatal signal|FATAL EXCEPTION|UnsatisfiedLinkError|vizij:|tts:" "$OUT/logcat-app.txt" | cut -c1-240 | head -40
if [ ! -s "$OUT/logcat-app.txt" ]; then fail "no log from the app (uid ${uid:-unknown})"; fi
if grep -qE "Caught" "$OUT/logcat-app.txt"; then fail "the app's log has a Caught line"; fi
if grep -qE "panicked at|Encountered a panic" "$OUT/logcat-app.txt"; then fail "the app panicked"; fi
if grep -qE "Fatal signal|FATAL EXCEPTION|UnsatisfiedLinkError" "$OUT/logcat-app.txt"; then fail "the app crashed"; fi
# The device client logs an enrollment once the pane's Enroll accepts it,
# which nothing presses here.
if grep -qE "enrolling the device as" "$OUT/logcat-app.txt"; then fail "the app enrolled the device unasked"; fi

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
echo "smoke passed: the face draws, animates and speaks; the settings pane opens, closes, keeps the background and shows the Device section"
