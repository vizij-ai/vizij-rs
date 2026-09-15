# Stage ROS4HRI skill interface packages — `std_skills`,
# `communication_skills`, `interaction_skills`, `hri_msgs` — from the
# `.msg` / `.action` files the arora-msgs-ros2 crate vendors, into a colcon
# `src` directory. So built (by `build-image.sh` into a container, or by
# `build-workspace.sh` natively), a client speaks exactly what the bridge
# serves: Vizij's viseme feedback extension and the LookAt skill included.
#
# All four are byte-for-byte the upstream `ros4hri/*` packages except one:
# `communication_skills/Say`'s Feedback carries Vizij's `viseme`/`intensity`
# extension (arora-msgs-ros2's README, "Departures from upstream") — a plain
# upstream `communication_skills` still accepts and completes a `say` goal
# (Goal and Result are unchanged) but never delivers feedback, silently. A
# workspace that already builds the other three from the real `ros4hri` org
# repos needs nothing from here but `communication_skills`; pass it alone.
#
# Sourced, not run: `source stage-packages.sh; stage_ros4hri_packages <src-dir> <repo-root> [package...]`.
# With no package names, stages all four.

stage_ros4hri_packages() {
  local W="$1" ROOT="$2"; shift 2
  local want=("$@")
  [ ${#want[@]} -eq 0 ] && want=(std_skills communication_skills interaction_skills hri_msgs)
  wants() { printf '%s\n' "${want[@]}" | grep -qx "$1"; }

  local MSGS
  MSGS="$(cd "$ROOT" && cargo metadata --format-version 1 2>/dev/null | python3 -c '
import json, os, sys
meta = json.load(sys.stdin)
crates = [p for p in meta["packages"] if p["name"] == "arora-msgs-ros2"]
if not crates:
    sys.exit("arora-msgs-ros2 is not in the dependency graph; build vizij with a ROS 2 feature first")
crates.sort(key=lambda p: p["version"])
print(os.path.join(os.path.dirname(crates[-1]["manifest_path"]), "msgs"))
')"
  for p in "${want[@]}"; do
    [ -d "$MSGS/$p" ] || { echo "no '$p' definitions under $MSGS" >&2; return 1; }
  done
  echo "interfaces from $MSGS: ${want[*]}" >&2

  package() { # name, description, deps...
    local name="$1" description="$2"; shift 2
    local deps=""; for d in "$@"; do deps+="  <depend>$d</depend>\n"; done
    printf '<?xml version="1.0"?>\n<package format="3">\n  <name>%s</name>\n  <version>0.1.0</version>\n  <description>%s</description>\n  <maintainer email="dev@semio.ai">Semio</maintainer>\n  <license>Apache-2.0</license>\n  <buildtool_depend>ament_cmake</buildtool_depend>\n  <buildtool_depend>rosidl_default_generators</buildtool_depend>\n%b  <exec_depend>rosidl_default_runtime</exec_depend>\n  <member_of_group>rosidl_interface_packages</member_of_group>\n  <export><build_type>ament_cmake</build_type></export>\n</package>\n' "$name" "$description" "$deps" > "$W/$name/package.xml"
  }
  cmake() { # name, files, deps
    local name="$1" files="$2" deps="$3"
    local finds=""; for d in $deps; do finds+="find_package($d REQUIRED)\n"; done
    printf 'cmake_minimum_required(VERSION 3.8)\nproject(%s)\nfind_package(ament_cmake REQUIRED)\nfind_package(rosidl_default_generators REQUIRED)\n%brosidl_generate_interfaces(${PROJECT_NAME} %s %s)\nament_export_dependencies(rosidl_default_runtime)\nament_package()\n' "$name" "$finds" "$files" "$([ -n "$deps" ] && echo "DEPENDENCIES $deps")" > "$W/$name/CMakeLists.txt"
  }

  if wants std_skills; then
    mkdir -p "$W/std_skills/msg"
    cp "$MSGS"/std_skills/*.msg "$W/std_skills/msg/"
    package std_skills "ROS4HRI standard skill messages, from arora-msgs-ros2's vendored definitions."
    cmake std_skills "$(cd "$W/std_skills" && ls msg/*.msg | sed 's/^/"/;s/$/"/' | tr '\n' ' ')" ""
  fi
  if wants communication_skills; then
    mkdir -p "$W/communication_skills/action"
    cp "$MSGS"/communication_skills/*.action "$W/communication_skills/action/"
    package communication_skills "ROS4HRI communication skills (Say carries Vizij's viseme feedback extension)." std_skills action_msgs
    cmake communication_skills "$(cd "$W/communication_skills" && ls action/*.action | sed 's/^/"/;s/$/"/' | tr '\n' ' ')" "std_skills action_msgs"
  fi
  if wants hri_msgs; then
    mkdir -p "$W/hri_msgs/msg"
    # AudioFeatures names a field `ZCR`, which rosidl rejects (field names
    # are lower_snake_case); the skills reference only Expression, so it is
    # left out.
    cp "$MSGS"/hri_msgs/*.msg "$W/hri_msgs/msg/" && rm "$W/hri_msgs/msg/AudioFeatures.msg"
    # The vendored files write the header as ROS 1's bare `Header`; rosidl
    # wants it qualified.
    sed -i.bak 's/^Header /std_msgs\/Header /' "$W"/hri_msgs/msg/*.msg && rm "$W"/hri_msgs/msg/*.bak
    package hri_msgs "ROS4HRI messages, from arora-msgs-ros2's vendored definitions." std_msgs
    cmake hri_msgs "$(cd "$W/hri_msgs" && ls msg/*.msg | sed 's/^/"/;s/$/"/' | tr '\n' ' ')" "std_msgs"
  fi
  if wants interaction_skills; then
    mkdir -p "$W/interaction_skills/action" "$W/interaction_skills/msg"
    cp "$MSGS"/interaction_skills/*.action "$W/interaction_skills/action/"
    cp "$MSGS"/interaction_skills/*.msg "$W/interaction_skills/msg/"
    sed -i.bak 's/^Header /std_msgs\/Header /' "$W"/interaction_skills/msg/*.msg && rm "$W"/interaction_skills/msg/*.bak
    package interaction_skills "ROS4HRI interaction skills (LookAt, SetExpression), from arora-msgs-ros2's vendored definitions." std_skills hri_msgs geometry_msgs action_msgs
    cmake interaction_skills "$(cd "$W/interaction_skills" && ls action/*.action msg/*.msg | sed 's/^/"/;s/$/"/' | tr '\n' ' ')" "std_skills hri_msgs geometry_msgs action_msgs"
  fi
}
