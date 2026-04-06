# Sandbox

OpenPinch exposes a platform-aware sandbox contract:

- Linux: Firecracker + `jailer` + seccomp
- macOS: virtualization backend contract
- Windows: Hyper-V backend contract

## Linux Firecracker Flow

The Linux backend expects:

- `firecracker` on `PATH` or configured explicitly
- `jailer` on `PATH` or configured explicitly
- A guest kernel image
- A guest root filesystem image
- An optional seccomp JSON policy

For each execution request the backend:

1. Creates an ephemeral workspace
2. Stages a task payload
3. Starts `jailer`
4. Configures the microVM through the Firecracker API socket
5. Boots the guest and waits for completion
6. Collects result artifacts and destroys the workspace

Use `openpinch start` or `openpinch execute` to surface sandbox doctor errors early.

