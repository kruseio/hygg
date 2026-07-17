# Contributing

Thank you for your interest in contributing to hygg!

## Contributor License Agreement (CLA)

> 🛡️ By submitting a pull request or contribution, you agree to the [Contributor License Agreement (CLA)](./CLA.md).

This means you permit us to use, license, and relicense your contributions — including under commercial terms — as outlined in the CLA.

You **do not need to sign anything** — submitting a PR or contribution implies agreement to the CLA terms.

## Before you submit

- hygg is a multi-license workspace; see [LICENSING.md](./LICENSING.md) for which license covers which crate, and keep changes within the license of the crate(s) you touch — don't move AGPL code into an MIT crate, and don't import AGPL code into `hygg-server`.
- Authored Rust source files must stay **≤ 300 lines** (enforced by `tools/loc-gate.sh`) — split larger modules.
- Write [Conventional Commits](https://www.conventionalcommits.org): `<type>[(scope)][!]: <description>`, e.g. `fix(server): don't drop the last page of a PDF`. This is not a style rule — the type is what the release is computed from. `tools/cliff.toml` turns these messages into the changelog that becomes the tag annotation and the release notes, and `git cliff --bumped-version` reads the same types to decide whether the next tag is a patch, a minor or a major, so a commit written outside the spec lands in the code and then appears in neither. The `commit-msg` hook below rejects those, and lists the accepted types when it does. Pick the type that matches what your diff actually does: a feature committed as `chore:` is a release that under-reports itself. See [Releases](docs/pages/release.md) for how the version falls out of it.
- Install the hooks once, after cloning: `git config core.hooksPath tools/hooks`. That gives you two. A `commit-msg` hook checking the message against the spec above — it costs nothing and runs on every commit. And a pre-push hook running `./tools/ci.sh fast` — fmt, loc, clippy and test — so the cheap half of CI tells you what it thinks before the runners do. That one is a subset, not the suite: tts, udeps, wasm and tauri still run on CI and can still surprise you. `git commit --no-verify` and `git push --no-verify` skip them when you need them skipped.
  - **A note on trust.** Pointing `core.hooksPath` at the worktree means the hook that runs on `git push` is whatever the *currently checked-out branch* says it is — and so is the `tools/ci.sh` it invokes, and the `cargo` build scripts and proc-macros that runs. That is fine for your own branches; it is code execution on your machine for someone else's. If you fetch and check out an untrusted pull-request branch to review it, don't `git push` from it with the hook armed (`--no-verify`, or review it in a throwaway clone), and don't run `./tools/ci.sh` against it, until you have read what changed under `tools/` and in the dependency set. Reviewing a branch should not run it.
- Run the test suite before opening a pull request. Every check a pull request faces is a leg of `tools/ci.sh`, and each leg runs on its own — `./tools/ci.sh test`, `./tools/ci.sh clippy`, `./tools/ci.sh fmt`, and so on (the script lists them all in its header) — so a red job in CI is one command to reproduce locally. `./tools/ci.sh` with no argument runs every leg; nothing it does writes to your tree.
- `tools/ci-mutable.sh` is the maintainer's full pass, and the one script here that edits your working tree: it upgrades dependencies, applies `cargo fix`, and formats the workspace before running the legs over the result — all of it on nightly, since that pass exists to look forward. That is a dependency bump, not a feature branch: keep it out of yours unless bumping is what you meant to do, and don't read a nightly-only clippy lint from it as something your pull request has to answer. `tools/ci.sh` on pinned stable is the gate.
