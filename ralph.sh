#!/usr/bin/env bash
set -euo pipefail

TASK="${TASK:-docs}"
BRANCH="${BRANCH:-ralph/docs}"
ITERS="${ITERS:-1}"
GOAL="${GOAL:-Improve documentation quality.}"

PROMPT_FILE=".ralph/LOOP_PROMPT.md"
PRIMARY_BACKLOG_FILE=".ralph/${TASK}-backlog.md"

mkdir -p .ralph/logs
touch .ralph/observations.md .ralph/decision-log.md

current_branch="$(git branch --show-current 2>/dev/null || true)"
if [ -n "$BRANCH" ] && [ "$current_branch" != "$BRANCH" ]; then
  git checkout -B "$BRANCH" >/dev/null 2>&1 || git checkout "$BRANCH"
fi

for i in $(seq 1 "$ITERS"); do
  ITER=$(printf "%02d" "$i")
  PROMPT="$(cat "$PROMPT_FILE")

ACTIVE GOAL:
$GOAL

TASK:
$TASK

PRIMARY BACKLOG:
$PRIMARY_BACKLOG_FILE

ITERATION:
iter-$ITER
"

  codex exec --json "$PROMPT" | tee ".ralph/logs/${TASK}-iter-${ITER}.jsonl"
done
