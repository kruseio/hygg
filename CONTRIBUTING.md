# Contributing

Thank you for your interest in contributing to hygg!

## Contributor License Agreement (CLA)

> 🛡️ By submitting a pull request or contribution, you agree to the [Contributor License Agreement (CLA)](./CLA.md).

This means you permit us to use, license, and relicense your contributions — including under commercial terms — as outlined in the CLA.

You **do not need to sign anything** — submitting a PR or contribution implies agreement to the CLA terms.

## Before you submit

- hygg is a multi-license workspace; see [LICENSING.md](./LICENSING.md) for which license covers which crate, and keep changes within the license of the crate(s) you touch — don't move AGPL code into an MIT crate, and don't import AGPL code into `hygg-server`.
- Authored Rust source files must stay **≤ 300 lines** (enforced by `tools/loc-gate.sh`) — split larger modules.
- Run the test suite before opening a pull request. Every check a pull request faces is a leg of `tools/ci.sh`, and each leg runs on its own — `./tools/ci.sh test`, `./tools/ci.sh clippy`, `./tools/ci.sh fmt`, and so on (the script lists them all in its header) — so a red job in CI is one command to reproduce locally. Note that `./tools/ci.sh` with *no* argument is the maintainer's full pass: it upgrades dependencies and rewrites your tree, which is not what you want on a feature branch.
