# Contributing

Thank you for your interest in contributing to hygg!

## Contributor License Agreement (CLA)

> 🛡️ By submitting a pull request or contribution, you agree to the [Contributor License Agreement (CLA)](./CLA.md).

This means you permit us to use, license, and relicense your contributions — including under commercial terms — as outlined in the CLA.

You **do not need to sign anything** — submitting a PR or contribution implies agreement to the CLA terms.

## Before you submit

- hygg is a multi-license workspace; see the [Licensing](./README.md#licensing) section for which license covers which crate, and keep changes within the license of the crate(s) you touch.
- Authored Rust source files must stay **≤ 300 lines** (enforced by `tools/loc-gate.sh`) — split larger modules.
- Run the test suite (and ideally `./tools/ci.sh`) before opening a pull request.
