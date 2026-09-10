#!/bin/bash
# Build the ROS 2 Jazzy + rmw_zenoh image carrying the ROS4HRI skill
# interfaces, from the `.msg` / `.action` files the arora-msgs-ros2 crate vendors
# (located through cargo, so the container speaks exactly the definitions the
# bridge serves — Vizij's extensions included). Usage: build-image.sh [tag]
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
TAG="${1:-vizij-ros4hri:jazzy}"
MSGS="$(cd "$HERE/../../../.." && cargo metadata --format-version 1 2>/dev/null | python3 -c '
import json, os, sys
meta = json.load(sys.stdin)
crates = [p for p in meta["packages"] if p["name"] == "arora-msgs-ros2"]
if not crates:
    sys.exit("arora-msgs-ros2 is not in the dependency graph; build vizij with a ROS 2 feature first")
crates.sort(key=lambda p: p["version"])
print(os.path.join(os.path.dirname(crates[-1]["manifest_path"]), "msgs"))
')"
[ -d "$MSGS/std_skills" ] && [ -d "$MSGS/communication_skills" ] || { echo "no ROS4HRI skill definitions under $MSGS" >&2; exit 1; }
echo "interfaces from $MSGS" >&2

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
W="$STAGE/ros4hri_ws/src"
mkdir -p "$W/std_skills/msg" "$W/communication_skills/action"
cp "$MSGS"/std_skills/*.msg "$W/std_skills/msg/"
cp "$MSGS"/communication_skills/*.action "$W/communication_skills/action/"

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
package std_skills "ROS4HRI standard skill messages, from arora-msgs-ros2's vendored definitions."
cmake std_skills "$(cd "$W/std_skills" && ls msg/*.msg | sed 's/^/"/;s/$/"/' | tr '\n' ' ')" ""
package communication_skills "ROS4HRI communication skills (Say carries Vizij's viseme feedback extension)." std_skills action_msgs
cmake communication_skills "$(cd "$W/communication_skills" && ls action/*.action | sed 's/^/"/;s/$/"/' | tr '\n' ' ')" "std_skills action_msgs"

cp "$HERE/Dockerfile" "$STAGE/Dockerfile"
docker build -f "$STAGE/Dockerfile" -t "$TAG" "$STAGE"
echo "built $TAG" >&2
