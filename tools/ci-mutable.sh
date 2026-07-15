#!/usr/bin/env bash

# The maintainer's local pass: every gate tools/ci.sh runs, plus the three steps
# a gate must not take. It refreshes the dependency tree, applies every fix that
# can be applied, formats the workspace, and runs the read-only legs over the
# result — so what it leaves behind is a tree that has been changed *and* then
# checked, which is the point of doing it in one command.
#
# It runs all of that on nightly, deliberately, where ci.sh runs it on pinned
# stable — this is the pass that looks forward, and the compiler is part of what
# it looks forward at. See TOOLCHAIN below for what that does and does not mean.
#
#   ./tools/ci-mutable.sh   The whole pass. No arguments; there is nothing here
#                           worth running a piece of. For one gate, or for a run
#                           that will not touch your tree, use tools/ci.sh.
#
# This file writes to your working tree, deliberately, in ways `git diff` will
# show you afterwards: cargo update/upgrade rewrite Cargo.lock and every
# manifest's version requirements, cargo fix edits source, cargo fmt reformats
# it. That is also why it cannot be the pull-request gate and why no runner calls
# it — a check that rewrites the tree has stopped testing the one that was
# proposed, and would pass a tree nobody would get.
#
# AGENTS: do not run this file unless you were explicitly asked to. Everything
# tools/ci.sh's header says applies here and then some: this one is slower (it
# resolves and rebuilds against fresh dependencies first) and it edits the tree
# under you, so a run started to check your own work can bury that work in an
# unrelated dependency bump you now have to explain in the diff. The pre-push
# hook runs tools/ci.sh, never this — pushing is not the moment to find out your
# dependencies moved.
#
# Deps: see tools/ci.sh's header.

set -Eeuo pipefail

# The toolchain pins (STABLE / NIGHTLY), the member exclusions (FORKS /
# HOST_ONLY) and the legs themselves (ci_audit, ci_clippy, ...) all come from
# ci.sh — sourced, so that the pass below runs the same commands the gate does
# rather than a copy of them that drifts. Sourcing also anchors cwd at the
# workspace root, and leaves LOCKED empty: only ci.sh's dispatcher sets
# --locked, and this pass must not have it. It opens by rewriting the lockfile on
# purpose, and `cargo upgrade` followed by the fork-manifest restore below leaves
# the lock legitimately ahead of those two manifests, which --locked would
# reject.
. "$(dirname "${BASH_SOURCE[0]}")/ci.sh"

# Everything here runs on nightly — the whole pass, not just the two legs that
# have no choice. This is the same decision as the `cargo update` below, made
# about the compiler instead of the dependencies: the point of this pass is to
# stand where the project is going and find out what breaks, while a fix is still
# cheap. Nightly clippy will refuse code the pinned gate accepts, and that is the
# feature — it is the lint the pin inherits the day it moves, found on a Tuesday
# of your choosing rather than under a release.
#
# What this is not is a second opinion about whether the tree is shippable.
# tools/ci.sh on pinned stable is the only thing that answers that, and a red leg
# here is not a red leg there. Fix it if it is real, note it if it is next year's
# problem, and do not move the pins in ci.sh to make this quiet.
TOOLCHAIN="$NIGHTLY"

# --- The steps that write -----------------------------------------------------

# Dependency refresh. The fork manifests are restored immediately after, because
# `cargo upgrade` has no per-member exclude and hygg-cff-parser /
# hygg-pdf-extract are meant to stay byte-identical to upstream (see FORKS in
# ci.sh). The legs that follow re-resolve the lock against the restored
# manifests, which is what reconciles the two before anything is committed.
ci_deps () {
  cargo $TOOLCHAIN update --verbose
  cargo $TOOLCHAIN upgrade --verbose
  git checkout -- packages/hygg-cff-parser/Cargo.toml \
                  packages/hygg-pdf-extract/Cargo.toml
}

# ci.sh's fmt leg --check's the tree; this writes it. Same pinned rustfmt (see
# NIGHTLY there, including why it must be nightly at all), so what this writes is
# exactly what that leg then accepts — the one step in this file whose toolchain
# is not a choice, and the one whose output the gate must agree with byte for
# byte.
ci_fmt_write () {
  cargo $NIGHTLY fmt --all
}

# --- The pass -----------------------------------------------------------------

ci_deps
ci_audit

cargo $TOOLCHAIN check --workspace "${HOST_ONLY[@]}"
cargo $TOOLCHAIN fix --allow-dirty --workspace "${HOST_ONLY[@]}"
ci_clippy
ci_fmt_write

# Run after fmt so the line counts reflect canonical formatting.
ci_loc

ci_test
ci_tts
ci_udeps

ci_wasm
ci_tauri
