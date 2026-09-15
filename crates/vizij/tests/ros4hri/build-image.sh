#!/bin/bash
# Build the ROS 2 Jazzy + rmw_zenoh image carrying the ROS4HRI skill
# interfaces (std_skills, communication_skills, interaction_skills, hri_msgs),
# staged by stage-packages.sh from the `.msg` / `.action` files the
# arora-msgs-ros2 crate vendors (located through cargo, so the container
# speaks exactly the definitions the bridge serves — Vizij's extensions
# included). Usage: build-image.sh [tag]
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
TAG="${1:-vizij-ros4hri:jazzy}"
source "$HERE/stage-packages.sh"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
stage_ros4hri_packages "$STAGE/ros4hri_ws/src" "$ROOT"

cp "$HERE/Dockerfile" "$STAGE/Dockerfile"
docker build -f "$STAGE/Dockerfile" -t "$TAG" "$STAGE"
echo "built $TAG" >&2
