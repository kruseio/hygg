#!/usr/bin/env bash

# Cut a release tag: work out the next version from the commits, bump the
# workspace to it, commit it, tag it with the changelog, push.
#
#   ./tools/.tag.sh          the version the commits ask for — the normal path
#   ./tools/.tag.sh 1.0.0    a version you are choosing yourself
#
# With no argument the version is not a judgement call: `git cliff
# --bumped-version` reads the conventional-commit types since the last tag and
# tools/cliff.toml's [bump] rules decide patch / minor / major. That is the
# other half of tools/hooks/commit-msg — the hook makes every commit carry a
# bump meaning, and this is what adds them up. An explicit argument overrides
# it, which is how 1.0.0 happens (deliberately, never as a side effect of a
# `feat!:`), and it is still held to every check below.
#
# The same commits produce the tag's annotation, so `git show 0.1.26` and the
# GitHub release say the same thing — release.yml reads the notes back off the
# tag rather than generating a second copy that could disagree.
#
# The tag is the trigger for four workflows at once — release.yml builds the
# artifacts, publish.yml pushes to crates.io, docker.yml pushes the image,
# pages.yml deploys the PWA — and the crates.io half of that is irreversible: a
# published version can never be replaced, only yanked. So everything checkable
# is checked here, before the push, and the push is the last thing that happens.
#
# 0.1.23 is why this exists. It was tagged without the version bump, so the tag
# said 0.1.23 while the tree said 0.1.22: every artifact would have carried a
# name for a version it did not contain, and tools/.publish.sh — which reads the
# version from Cargo.toml, not from the tag — would have skipped the entire
# publish as already-uploaded, succeeding while doing nothing. Nothing caught it
# because nothing was looking. release.yml's `guard` job is the same check on
# the CI side; this is the one that runs before the mistake reaches GitHub.
#
# Dot-prefixed like .publish.sh: not a ci.sh leg, a maintainer action.

set -Eeuo pipefail

# The version read, the manifest write, and every git command below want the
# workspace root, so anchor there rather than trusting the caller's directory.
cd "$(dirname "${BASH_SOURCE[0]}")/.."

die () {
  echo "error: $*" >&2
  exit 1
}

ASSUME_YES=false
ARGS=()
for arg in "$@"; do
  case "$arg" in
    -y | --yes) ASSUME_YES=true ;;
    -*) die "unknown option: $arg" ;;
    *) ARGS+=("$arg") ;;
  esac
done

VERSION="${ARGS[0]:-}"
[ "${#ARGS[@]}" -le 1 ] ||
  die "usage: ${0##*/} [-y] [version]   e.g. ${0##*/}, or ${0##*/} 1.0.0"

# Bare semver, no `v`. Not a house style: every tag-triggered workflow filters on
# "[0-9]+.[0-9]+.[0-9]+", so a `v0.1.23` tag pushes fine and then silently fires
# nothing at all — which is how every release up to 0.1.22 ended up with no
# artifacts. Reject here rather than let that happen quietly again.
#
# A function rather than a straight-line check because the version now arrives
# two ways — the argument, or git-cliff's answer below — and neither is trusted
# more than the other.
check_version () {
  case "$1" in
    v*) die "tags here are bare semver: use ${1#v}, not $1" ;;
  esac
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
    die "'$1' is not bare semver (X.Y.Z), which is all the workflows match"
}
[ -z "$VERSION" ] || check_version "$VERSION"

# The changelog and the version both come from git-cliff, so its absence is a
# stop rather than something to work around: a hand-written tag message is
# exactly the drift between the tag and the release that reading one from the
# other is meant to rule out.
CLIFF=(git-cliff --config tools/cliff.toml)
command -v git-cliff >/dev/null ||
  die "git-cliff is not installed — cargo install --locked git-cliff"

# --- Preconditions ------------------------------------------------------------

# A dirty tree means the release commit below would sweep up whatever else is
# open and ship it under a version bump's name.
[ -z "$(git status --porcelain)" ] ||
  die "working tree is dirty — commit or stash first, a release commit should carry only the bump"

branch="$(git rev-parse --abbrev-ref HEAD)"
[ "$branch" = main ] ||
  die "on '$branch': releases are cut from main, which is the branch CI gates"

# Fetch before judging: both the "is the tag taken" check and the "am I current"
# check below are answers about the remote, and a stale local view of it is
# worth nothing.
git fetch --quiet --tags origin

# After the fetch, never before: "the next version" is a question about the
# commits since the *last tag*, and asking it against a stale view of the tags
# answers for a release that already happened.
if [ -z "$VERSION" ]; then
  VERSION="$("${CLIFF[@]}" --bumped-version 2>/dev/null)" ||
    die "git-cliff could not work out the next version from the commits"
  # git-cliff echoes the last tag back when nothing since it warrants a bump.
  # The tag-exists check below would catch that anyway, but not legibly.
  [ -n "$VERSION" ] ||
    die "git-cliff returned no version"
  check_version "$VERSION"
  echo "the commits since $(git describe --tags --abbrev=0 2>/dev/null || echo "the start") ask for $VERSION"
fi

git rev-parse --verify --quiet "refs/tags/$VERSION" >/dev/null &&
  die "tag $VERSION already exists locally"
git ls-remote --exit-code --tags origin "refs/tags/$VERSION" >/dev/null 2>&1 &&
  die "tag $VERSION already exists on origin — a released version is not reusable"

# Tagging a commit origin does not have publishes artifacts built from code
# nobody can fetch; being behind means tagging without changes you think are in.
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] ||
  die "local main and origin/main disagree — pull/push until they match, then tag"

# Every member takes `version.workspace = true`, so [workspace.package]'s version
# is the version of all of them, and the first `version =` in the root manifest.
# Same read as tools/.publish.sh and release.yml's guard job.
CURRENT="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)"
[ -n "$CURRENT" ] || die "could not read the workspace version from Cargo.toml"

# Refuse to go backwards or sideways. `sort -V` orders versions rather than
# strings, so 0.1.9 sorts below 0.1.10 the way a human means it.
if [ "$VERSION" != "$CURRENT" ]; then
  older="$(printf '%s\n%s\n' "$CURRENT" "$VERSION" | sort -V | head -1)"
  [ "$older" = "$CURRENT" ] ||
    die "$VERSION is older than the tree's $CURRENT"
fi

# Already on crates.io is a warning, not a refusal: the artifacts and the crates
# are separate halves of a release, and re-cutting a tag to get the half that
# never shipped is legitimate — 0.1.22's crates went out while its GitHub
# release stayed empty. The publish leg skips what is already up, by design.
if curl -sf -o /dev/null \
  -H "User-Agent: hygg-tag (https://github.com/kruseio/hygg)" \
  "https://crates.io/api/v1/crates/hygg/$VERSION"; then
  echo "note: hygg $VERSION is already on crates.io; publish.yml will skip the"
  echo "      crates that are up and publish only what is missing."
fi

# --- The notes ----------------------------------------------------------------

# Rendered before the bump commit exists, which reads backwards but is not:
# cliff.toml skips `release:` commits, so the bump could never appear in its own
# notes anyway — and doing it here puts the changelog on screen *before* the
# confirmation rather than after it.
NOTES="$("${CLIFF[@]}" --unreleased --tag "$VERSION" 2>/dev/null)" ||
  die "git-cliff could not render the changelog for $VERSION"

# A tag annotation's first line is its subject, and git-cliff pads its render
# with blank lines: left alone, the subject would be empty and the version would
# land in the body. `$(...)` has already eaten the trailing newlines; this drops
# the leading ones.
NOTES="$(printf '%s\n' "$NOTES" | sed -e '/./,$!d')"

# One line is the version and nothing else — every commit since the last tag got
# filtered out, because they were all `release:` or none of them parsed. Either
# way the notes would say nothing, which is worth stopping for rather than
# publishing.
[ "$(printf '%s\n' "$NOTES" | wc -l | tr -d ' ')" -gt 1 ] ||
  die "no commits since the last tag would show up in the changelog — nothing to release"

# --- The bump -----------------------------------------------------------------

if [ "$VERSION" = "$CURRENT" ]; then
  echo "Cargo.toml is already at $VERSION — tagging HEAD without a bump commit."
  bump=false
else
  bump=true
fi

echo
echo "  tag:     $VERSION"
echo "  commit:  $(git rev-parse --short HEAD) $(git log -1 --pretty=%s)"
$bump && echo "  bump:    $CURRENT -> $VERSION"
echo
echo "  The tag annotation, and the release notes — the same text:"
echo
printf '%s\n' "$NOTES" | sed 's/^/    | /'
echo
echo "Pushing this tag builds the release artifacts, publishes to crates.io"
echo "(immutable), pushes the container image, and deploys the PWA."

# From /dev/tty rather than stdin, so that a `yes |` or a stray pipe cannot
# answer this on the maintainer's behalf. -y is the deliberate way to skip it.
if ! $ASSUME_YES; then
  read -r -p "Continue? [y/N] " reply </dev/tty
  case "$reply" in
    y | Y | yes | YES) ;;
    *) die "aborted — nothing was changed or pushed" ;;
  esac
fi

if $bump; then
  # Only the first `version =`, which is [workspace.package]'s: awk rather than
  # `sed -i`, whose in-place flag differs between GNU and BSD and so would work
  # on a runner and not on the maintainer's mac.
  awk -v v="$VERSION" '
    !done && /^version = "/ { sub(/"[^"]*"/, "\"" v "\""); done = 1 }
    { print }
  ' Cargo.toml > Cargo.toml.tag.tmp && mv Cargo.toml.tag.tmp Cargo.toml

  written="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)"
  [ "$written" = "$VERSION" ] ||
    die "the bump did not take: Cargo.toml still reads '$written'"

  # The lockfile pins the members' own versions too, so it is now stale — and
  # every CI leg builds with --locked, which fails on a stale lock rather than
  # fixing it. --workspace touches only the members, leaving the dependency
  # resolution this release was tested against exactly as it is.
  cargo update --workspace --quiet

  # Prove the pair is consistent before committing it, since --locked is what CI
  # will judge it by. Cheap: metadata resolves, it does not build.
  cargo metadata --locked --format-version 1 >/dev/null ||
    die "Cargo.lock is out of sync with Cargo.toml after the bump"

  git add Cargo.toml Cargo.lock
  git commit --quiet -m "release: $VERSION"
  echo "committed the bump"
fi

# --- The push -----------------------------------------------------------------

# Commit first, tag second. The tag would carry the commit with it either way,
# but this order means a rejected push leaves no tag behind to clean up, and
# main never points at something the tag does not.
#
# `if`, not `$bump && git push ... && echo ...`: errexit is ignored for every
# command of an AND-OR list but the last, so a rejected push — a protected
# branch, a race, no network — would not have stopped the script. It would have
# gone on to tag anyway, and that tag is the trigger for four workflows and an
# irreversible crates.io publish of a commit origin/main does not have. The
# whole point of the ordering above is that the push is what gates the tag, so
# the push has to be able to fail.
if $bump; then
  git push --quiet origin main
  echo "pushed main"
fi

# The annotation is not decoration: release.yml reads it back with
# `%(contents:body)` and it *is* the release notes. An annotated tag (-a) is
# what makes that possible at all — a lightweight tag has no message to read.
git tag -a "$VERSION" -m "$NOTES"
git push --quiet origin "refs/tags/$VERSION"
echo "pushed tag $VERSION"

echo
echo "Watch it land:"
echo "  gh run list --limit 5"
echo "  gh release view $VERSION"
