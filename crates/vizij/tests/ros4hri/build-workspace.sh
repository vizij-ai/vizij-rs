#!/bin/bash
# Stage and colcon-build the ROS4HRI skill interface packages into a plain ROS
# workspace — no Docker. For joining a real ROS graph on Linux, where ROS 2
# and rmw_zenoh_cpp are already installed: see docs/ros4hri.md ("With
# rmw_zenoh"). Usage: build-workspace.sh [workspace-dir] (default ~/ros4hri_ws)
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
WS="${1:-$HOME/ros4hri_ws}"
source "$HERE/stage-packages.sh"

mkdir -p "$WS/src"
stage_ros4hri_packages "$WS/src" "$ROOT"

source "/opt/ros/${ROS_DISTRO:-jazzy}/setup.bash"
(cd "$WS" && colcon build --merge-install)
echo "built $WS — source it with: source $WS/install/setup.bash" >&2
