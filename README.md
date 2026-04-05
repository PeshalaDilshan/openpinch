# 🦀 OpenPinch — Autonomous AI Agent

**A faster, more secure, and truly local-first autonomous AI assistant.**

OpenPinch is your personal AI agent that runs entirely on your own hardware (Mac, Windows, Linux). It communicates through the messaging apps you already use (WhatsApp, Telegram, Discord, Slack, Signal, and many more), performs real-world tasks, and operates proactively 24/7 — all while keeping your data private.

Built with a high-performance **Rust core**, efficient **Go messaging & scheduling gateway**, and beautiful cross-platform **Flutter** companion apps, with **kernel-level isolation** (Firecracker microVMs + seccomp) by default.

### Why OpenPinch?
- **Superior Performance** — Lower resource usage and latency than Node.js-based agents
- **Stronger Security** — Mandatory kernel-level sandboxing (no more incomplete opt-in isolation)
- **Better Usability** — One-command install + native desktop/mobile/web apps via Flutter
- **Full Control** — 100% open source, self-hosted, no subscriptions, no data leaving your machine

### Key Features
- Multi-platform messaging integration (20+ apps)
- Modular skills system with cryptographic verification
- Persistent encrypted memory and proactive autonomy
- Kernel-level sandboxing for every tool and skill
- Multi-agent coordination
- Beautiful companion apps (desktop, mobile, web) built with Flutter

### Quick Start
```bash
# Clone the repo
git clone https://github.com/PeshalaDilshan/openpinch.git
cd openpinch

# Setup (installs Rust, Go, Flutter tooling)
make setup

# Run in development mode (core + gateway + UI with hot reload)
make dev
```

For detailed installation and setup, see the [docs/](docs/) folder or [Getting Started Guide](docs/getting-started.md) (coming soon).

### Architecture
- **core/** — Rust (performance & security-critical engine)
- **gateway/** — Go (messaging, scheduler, concurrency)
- **ui/** — Flutter/Dart (cross-platform companion apps)
- **proto/** — Shared gRPC definitions
- Kernel-level isolation with Firecracker microVMs

### Tech Stack
- **Rust** — Core engine, sandboxing
- **Go** — High-concurrency networking & scheduling
- **Flutter (Dart)** — Desktop, mobile & web UI
- **gRPC** — Clean inter-component communication

### Status
🚧 Early development stage — architecture and monorepo structure finalized.  
Contributions and early feedback are very welcome!

### Links
- [Documentation](docs/) (will be expanded)
- [Contributing](CONTRIBUTING.md)
- [Issues](https://github.com/PeshalaDilshan/openpinch/issues)
- [Discussions](https://github.com/PeshalaDilshan/openpinch/discussions) (recommended for questions)

---

**OpenPinch** — Built to be the better alternative.  
Made with ❤️ for privacy, performance, and real autonomy.
