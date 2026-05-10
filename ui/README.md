# OpenPinch Desktop

This directory contains the Flutter desktop client for OpenPinch.

The desktop app is the primary operator shell for Linux, macOS, and Windows. It launches the bundled runtime through `openpinch desktop host`, then controls the local system through CLI and gRPC-backed runtime surfaces instead of a browser-facing gateway UI.

Local workflow:

```bash
cd ui
flutter pub get
flutter analyze
flutter test
flutter build linux --release
```

Development notes:

- `OPENPINCH_BIN=/path/to/openpinch flutter run -d linux` forces the desktop app to use a specific CLI binary.
- The app expects the `openpinch` binary to be bundled beside the desktop executable in release builds.
- The local desktop chat path is the `desktop` connector, not `webchat`.
