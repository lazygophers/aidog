#!/usr/bin/env bash
# Watch the aidog Release workflow run for the current local HEAD commit.
#
# Usage (run in background):
#   bash .claude/skills/aidog-release/scripts/watch-release.sh [head_sha]
#
# Matches the run by HEAD sha so it never latches onto the previous release run.
# On failure it appends the failed-step log tail to the same output file.
# Exit: 0 = success, 1 = failure, 2 = no run found within the wait window.
set -uo pipefail

cd "$(git rev-parse --show-toplevel)"

SHA="${1:-$(git rev-parse HEAD)}"
VERSION="$(tr -d '[:space:]' < .version)"
OUT="/tmp/aidog-release-${VERSION}.log"

echo "[watch-release] version=${VERSION} sha=${SHA}" | tee "$OUT"

RUN_ID=""
# GitHub takes a few seconds to register the run after a push; wait up to 5 minutes.
for _ in $(seq 1 60); do
  RUN_ID="$(gh run list --workflow=release.yml --limit 20 \
    --json databaseId,headSha \
    -q "[.[] | select(.headSha == \"${SHA}\")] | .[0].databaseId" 2>/dev/null || true)"
  [ -n "$RUN_ID" ] && [ "$RUN_ID" != "null" ] && break
  RUN_ID=""
  sleep 5
done

if [ -z "$RUN_ID" ]; then
  echo "[watch-release] no run found for ${SHA} after 5 minutes" | tee -a "$OUT"
  echo "[watch-release] did the push land on master, and does the diff touch .version or release.yml?" | tee -a "$OUT"
  exit 2
fi

echo "[watch-release] run id=${RUN_ID}" | tee -a "$OUT"
# gh run watch 对瞬时网络错（EOF / connection reset）也返回非 0（2026-10-03 两次误杀），
# 终态只认 gh run view 的 conclusion：watch 失败就重挂，最多 10 轮。
STATUS=1
for _ in $(seq 1 10); do
  gh run watch "$RUN_ID" --exit-status --interval 30 >> "$OUT" 2>&1 && STATUS=0 || STATUS=1
  CONCLUSION="$(gh run view "$RUN_ID" --json status,conclusion \
      -q '.status + "/" + (.conclusion // "")' 2>/dev/null || true)"
  case "$CONCLUSION" in
    completed/success) STATUS=0; break ;;
    completed/*)       STATUS=1; break ;;
  esac
done

if [ "$STATUS" -ne 0 ]; then
  {
    echo
    echo "===== failed step log (tail 120) ====="
    gh run view "$RUN_ID" --log-failed 2>&1 | tail -120
  } >> "$OUT"
  echo "[watch-release] FAILED — see ${OUT}"
  exit 1
fi

{
  echo
  echo "===== release assets ====="
  gh release view "v${VERSION}" --json assets -q '.assets[].name' 2>&1
} >> "$OUT"

echo "[watch-release] SUCCESS — see ${OUT}"
