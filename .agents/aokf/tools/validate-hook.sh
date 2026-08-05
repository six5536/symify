#!/usr/bin/env bash
# Claude Code PostToolUse hook: after an Edit/Write under knowledge/, run the
# AOKF validator. Exit 2 feeds the findings back to the agent as a blocking
# error; anything outside the bundle is ignored.
set -euo pipefail

root="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../../.." && pwd)}"

path="$(python3 -c 'import json,sys; print(json.load(sys.stdin).get("tool_input",{}).get("file_path",""))' 2>/dev/null || true)"
case "$path" in
  "$root"/knowledge/*|knowledge/*) ;;
  *) exit 0 ;;
esac

if ! out="$(python3 "$root/.agents/aokf/tools/validator.py" "$root/knowledge" --level 2 2>&1)"; then
  echo "AOKF validation failed after editing $path — fix before continuing:" >&2
  echo "$out" >&2
  exit 2
fi
