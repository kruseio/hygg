# Development

### [<-](../README.md)

Test that it works
```sh
cargo run --release -- test-data/pdf/pdfreference1.7old.pdf 
```

## Commits
Every commit follows [Conventional Commits](https://www.conventionalcommits.org),
because the type on it is an input to the release rather than a label on it —
see [Contributing](../../CONTRIBUTING.md). Arm the hook that checks it, once:
```sh
git config core.hooksPath tools/hooks
```

## Cutting a release
Maintainers only, and irreversible in part: the tag publishes to crates.io,
where a version can be yanked but never replaced.

```sh
cargo install --locked git-cliff   # once
./tools/.tag.sh                    # the version the commits ask for
```

`tools/.tag.sh` works out the next version from the conventional-commit types
since the last tag, bumps the workspace to it, renders the changelog, and shows
you all of it before anything is pushed. It refuses a dirty tree, a branch other
than main, a local main that disagrees with origin, a tag that already exists,
and a version older than the tree's.

Pass a version to override the computed one — `./tools/.tag.sh 1.0.0` — which is
how a major happens deliberately rather than because someone wrote `feat!:`.

The tag is the trigger for four workflows: `release.yml` (artifacts),
`publish.yml` (crates.io), `docker.yml` (the server image) and `pages.yml` (the
web app). See [Releases](release.md) for what comes out and where the version and
the changelog come from.

To look without tagging — `--config` matters, since a bare `git cliff` silently
answers from its own defaults instead of this repo's:
```sh
git cliff --config tools/cliff.toml --unreleased       # the notes it would carry
git cliff --config tools/cliff.toml --bumped-version   # the version it would be
gh run list --limit 5                                  # watch it land
```
