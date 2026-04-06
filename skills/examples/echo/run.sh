#!/bin/sh
set -eu

payload="${OPENPINCH_SKILL_ARGS_JSON:-{}}"
message="$(printf '%s' "$payload" | sed -n 's/.*"message"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"

if [ -z "$message" ]; then
  message="hello from the OpenPinch echo skill"
fi

printf 'echo-skill: %s\n' "$message"
