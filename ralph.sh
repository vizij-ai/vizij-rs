#!/usr/bin/env bash
set -euo pipefail

TASK="${TASK:-docs}"
BRANCH="${BRANCH:-ralph/docs}"
ITERS="${ITERS:-1}"
GOAL="${GOAL:-Improve documentation quality.}"
SCOPE="${SCOPE:-}"
BUDGET_FILES="${BUDGET_FILES:-}"

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
  START_HEAD="$(git rev-parse HEAD)"
  PROMPT="$(cat "$PROMPT_FILE")

ACTIVE GOAL:
$GOAL

TASK:
$TASK

PRIMARY BACKLOG:
$PRIMARY_BACKLOG_FILE
"

  if [ -n "$SCOPE" ]; then
    PROMPT="$PROMPT
SCOPE HINT:
$SCOPE
"
  fi

  if [ -n "$BUDGET_FILES" ]; then
    PROMPT="$PROMPT
BUDGET FILES:
$BUDGET_FILES
"
  fi

  PROMPT="$PROMPT
ITERATION:
iter-$ITER
"

  codex exec --json "$PROMPT" | tee ".ralph/logs/${TASK}-iter-${ITER}.jsonl"

  END_HEAD="$(git rev-parse HEAD)"
  if [ "$START_HEAD" = "$END_HEAD" ]; then
    SCOPE_TAG="${SCOPE:-auto}"
    COMMIT_MSG="ralph($TASK iter-$ITER): ${SCOPE_TAG} docs"
    if git diff --quiet && git diff --cached --quiet; then
      git commit --allow-empty -m "$COMMIT_MSG"
    else
      git add -A
      git commit -m "$COMMIT_MSG"
    fi
  fi
done
