#!/bin/bash
# Stage and colcon-build the ROS4HRI skill interface packages into a plain ROS
# workspace — no Docker. For joining a real ROS graph on Linux, where ROS 2
# and rmw_zenoh_cpp are already installed: see docs/ros4hri.md ("With
# rmw_zenoh"). Usage:
#   build-workspace.sh [--only pkg1,pkg2,...] [workspace-dir]
# (default: all four packages, into ~/ros4hri_ws)
#
# A workspace that already builds std_skills/interaction_skills/hri_msgs from
# the real ros4hri org repos needs nothing else from here but
# `--only communication_skills` — source that workspace, then source this
# one last so its communication_skills (Vizij's viseme/intensity extension)
# shadows any plain upstream copy on AMENT_PREFIX_PATH.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
ONLY=()
if [ "${1:-}" = "--only" ]; then
  IFS=',' read -ra ONLY <<< "$2"
  shift 2
fi
WS="${1:-$HOME/ros4hri_ws}"
source "$HERE/stage-packages.sh"

mkdir -p "$WS/src"
if [ ${#ONLY[@]} -gt 0 ]; then
  stage_ros4hri_packages "$WS/src" "$ROOT" "${ONLY[@]}"
else
  stage_ros4hri_packages "$WS/src" "$ROOT"
fi

source "/opt/ros/${ROS_DISTRO:-jazzy}/setup.bash"
(cd "$WS" && colcon build --merge-install)
echo "built $WS — source it with: source $WS/install/setup.bash" >&2
