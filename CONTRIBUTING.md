# Contributing to OpenPinch

Thank you for considering contributing to **OpenPinch**!  
We welcome contributions of all kinds — code, documentation, bug reports, feature ideas, and more.

## Code of Conduct
By participating, you agree to our [Code of Conduct](CODE_OF_CONDUCT.md) (create this file if needed — keep it friendly and inclusive).

## How to Contribute

### 1. Reporting Bugs or Suggesting Features
- Open an issue in the [Issues tab](https://github.com/PeshalaDilshan/openpinch/issues).
- Use clear titles and describe the problem or idea with as much detail as possible (steps to reproduce, expected vs actual behavior, screenshots, etc.).

### 2. Submitting Code Changes (Pull Requests)
1. Fork the repository.
2. Create a new branch for your feature/fix (`git checkout -b feature/amazing-idea`).
3. Make your changes following the project structure:
   - Rust changes → `core/`
   - Go changes → `gateway/`
   - UI changes → `ui/`
   - Shared protocols → `proto/`
4. Test your changes (`make test`).
5. Commit with clear messages (e.g., "feat(core): add kernel sandbox manager").
6. Push to your fork and open a Pull Request against the `dev` branch.

### Development Setup
```bash
git clone https://github.com/PeshalaDilshan/openpinch.git
cd openpinch
make setup
make dev