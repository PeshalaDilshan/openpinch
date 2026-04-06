# OpenPinch Operator

This directory contains the OpenPinch Kubernetes operator skeleton for enterprise deployments.

Current scope:
- Watches `OpenPinchAgent` custom resources.
- Renders deployment intent for the gateway, policy bundle, and connector secret references.
- Keeps the workstation-first local mode outside Kubernetes unchanged.

The implementation is intentionally lightweight in this repo rebuild and is structured so a controller-runtime manager can replace the current reconciliation loop without changing the CRD shape.
