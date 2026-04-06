#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <openclaw-config-dir> [output-report.json]" >&2
  exit 1
fi

SOURCE_DIR="$1"
REPORT_PATH="${2:-migration-report.json}"

if [[ ! -d "$SOURCE_DIR" ]]; then
  echo "source directory not found: $SOURCE_DIR" >&2
  exit 1
fi

cat >"$REPORT_PATH" <<EOF
{
  "source": "$(cd "$SOURCE_DIR" && pwd)",
  "generated_at": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "notes": [
    "OpenClaw migration is dry-run only in this repository rebuild.",
    "Connector mappings should be reviewed before enabling production tokens.",
    "Unsigned skills must be re-packaged and signed for OpenPinch v2."
  ],
  "detected_files": [
$(find "$SOURCE_DIR" -maxdepth 2 -type f | sed 's/.*/    "&"/' | paste -sd ",\n" -)
  ]
}
EOF

echo "wrote migration report to $REPORT_PATH"
