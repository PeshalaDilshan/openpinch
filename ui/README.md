# OpenPinch UI

This directory contains the Flutter web control surface for OpenPinch.

It targets the gateway HTTP API and event stream exposed by the Go server. A release build is emitted to `ui/build/web`, which the gateway serves when `gateway.web.enabled = true`.

Local workflow:

```bash
cd ui
flutter pub get
flutter analyze
flutter test
flutter build web --release
```
