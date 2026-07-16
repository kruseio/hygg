#!/usr/bin/env bash

# Cut a release tag: bump the workspace version to $1, commit it, tag it, push.
#
#   ./tools/.tag.sh 0.1.23
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
[ -n "$VERSION" ] ||
  die "usage: ${0##*/} [-y] <version>   e.g. ${0##*/} 0.1.23"

# Bare semver, no `v`. Not a house style: every tag-triggered workflow filters on
# "[0-9]+.[0-9]+.[0-9]+", so a `v0.1.23` tag pushes fine and then silently fires
# nothing at all — which is how every release up to 0.1.22 ended up with no
# artifacts. Reject here rather than let that happen quietly again.
case "$VERSION" in
  v*) die "tags here are bare semver: use ${VERSION#v}, not $VERSION" ;;
esac
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
  die "'$VERSION' is not bare semver (X.Y.Z), which is all the workflows match"

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

git tag -a "$VERSION" -m "$VERSION"
git push --quiet origin "refs/tags/$VERSION"
echo "pushed tag $VERSION"

echo
echo "Watch it land:"
echo "  gh run list --limit 5"
echo "  gh release view $VERSION"
