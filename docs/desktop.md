# Desktop Client

OpenPinch is desktop-first.

The Flutter app in `ui/` is the primary control surface for Linux, macOS, and Windows. It does not depend on gateway-hosted browser routes. Instead, it launches and monitors the local runtime through `openpinch desktop host`.

## Host Lifecycle

- `openpinch desktop host` starts the Rust engine and Go gateway sidecars for the desktop app.
- `openpinch desktop health` reports whether the host state exists and whether the local gateway is reachable.
- `openpinch desktop shutdown` requests a clean stop by writing a local shutdown signal that the host loop consumes.

The host writes state into the runtime directory under the standard OpenPinch OS-specific state path. This is how the desktop app reconnects to an already-running host instead of spawning duplicates.

## Local Desktop Messaging

- The local desktop chat path uses the `desktop` connector.
- Inbound desktop chat goes through `openpinch message post desktop ...`.
- Outbound connector delivery still uses `openpinch message send ...` for real external channels.

## Logs And Troubleshooting

- Runtime logs still go to the normal OpenPinch log file in the state log directory.
- `openpinch doctor` reports desktop host configuration alongside models, sandbox state, and connector readiness.
- If the desktop app cannot find the CLI sidecar automatically, set `OPENPINCH_BIN=/absolute/path/to/openpinch` before launching the app.

## Packaging

The intended release shape is:

- native desktop executable from `ui/`
- bundled `openpinch` CLI sidecar
- bundled `openpinch-gateway` sidecar

All three should live in the same installed app bundle or release directory so the desktop app can resolve and launch the runtime locally without a separate manual install step.
