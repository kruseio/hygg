# hygg-server

Self-hostable, multi-tenant **sync server** for the [hygg](https://github.com/kruseio/hygg)
reader. Sync your documents, reading progress, bookmarks, highlights and notes
across devices — or run hygg fully offline and never touch a server at all.

Built as a single Rust binary: `axum` + `tokio` + `sea-orm`, server-rendered
UI, no Node build step. Stores everything in **SQLite** — a file, no extra
services to run.

> Status: progress + bookmark/highlight/note sync, documents/blobs, per-device
> auth and document scopes, request body caps, auth rate limiting, SSE push,
> organizations with groups and permissions, peer document sharing, export/import,
> and the multi-tenant data model are implemented and tested. The server-rendered
> web UI includes signup, password/recovery login, full WebAuthn passkey
> registration and login ceremonies, browser session management, user
> device-token creation, a `/docs` help center, and an admin backoffice for users,
> roles, disabling users, device permissions/tokens, recovery passwords, and
> passkey revocation.

## One-command self-host (SQLite)

```sh
cd packages/hygg-server
cp .env.example .env           # optional; sensible defaults work as-is
# set ADMIN_BOOTSTRAP_EMAIL / ADMIN_BOOTSTRAP_PASSWORD in .env to create an admin
docker compose up --build -d
```

The server listens on `http://localhost:3032`. Everything it stores — the SQLite
database, document blobs (rows in it, not files) and logs — persists in
`./hygg-data`. Open <http://localhost:3032/> in a browser for the web UI, or
check it from the shell:

```sh
curl http://localhost:3032/health      # {"status":"ok"}
```

> Use an explicit `http://` URL. Browsers may auto-upgrade a bare
> `localhost:3032` to HTTPS, which this server doesn't serve — that shows up as
> `ERR_CONNECTION_REFUSED`.

### Staging alongside production

`compose.staging.yml` is a full second instance — its own database, network,
image and port — that runs beside prod from this same clone:

```sh
cp .env.staging.example .env.staging
docker compose -f compose.staging.yml --env-file .env.staging up --build -d
```

It listens on `http://localhost:3033` and stores its data in
`./hygg-data-staging`, so a staging run cannot reach the production database.
Always pass `--env-file .env.staging`: without it Compose interpolates from the
prod `.env` and the two stacks collide on `PORT`.

| | production | staging |
|---|---|---|
| compose file | `compose.yml` (default) | `compose.staging.yml` |
| config | `.env` | `.env.staging` |
| project | `hygg-server` | `hygg-server-staging` |
| port | `3032` | `3033` |
| data | `./hygg-data` | `./hygg-data-staging` |

### The data directory

`hygg-data` is hygg's tree, and the server claims it on first use by writing a
`.hygg-server` marker file. On every start it checks for that marker, and
**refuses to boot** on a directory that is non-empty and holds nothing of
hygg's — printing what it found and how to fix it, rather than writing its tree
into whatever was there. A bind mount is one string in a shell command; this is
what stops a typo in it from being your problem.

The server is the tree's principal occupant, not its only one: `hygg-logs/` is
shared, and sibling tools log beside it there (`packages/hygg-pwa/serve_dist.py`
writes `hygg-logs/hygg-pwa/`). So a `hygg-server.db` **or** a `hygg-logs/`
directory both count as hygg's — serving the PWA from a fresh checkout before
the server has ever run leaves a tree the server then adopts on its first start,
rather than refusing. Neither name belongs to anything but hygg, so neither can
be mistaken for a stranger's file.

The name is deliberately specific: the mount is created in whatever directory
you ran Docker from, and `data` is a name that is often already taken.

> Upgrading from a deployment that used `data`? Nothing moves on its own — stop
> the server, `mv data hygg-data`, and update your `-v` flag. The server adopts
> and marks an existing directory that holds a `hygg-server.db`, so a mount left
> pointing at the old path keeps working too.

### Prebuilt image

Every release publishes a multi-arch image (`linux/amd64` + `linux/arm64`) to
GHCR, so a self-host needs no toolchain and no compile:

```sh
docker run -d -p 3032:3032 -v "$PWD/hygg-data:/app/data" \
  ghcr.io/kruseio/hygg-server:latest
```

The mount's host side (`$PWD/hygg-data`) is yours to name and move; `/app/data`
is the image's own path and should stay as it is.

Tags are `:latest`, `:0.1.21` (pin this) and `:0.1`. To use it from the compose
file above, replace the `build:` block with
`image: ghcr.io/kruseio/hygg-server:latest` — the volume and environment stay as
they are.

### Run from source (cargo)

```sh
cd packages/hygg-server
cargo run -p hygg-server       # loads .env from here; data lands in ./hygg-data
```

> **macOS firewall note.** rustc/cargo leave the binary "linker-signed", which
> the Application Firewall treats as unsigned and blocks for **LAN** connections
> — `localhost:3032` works but `http://<your-lan-ip>:3032` gets a reset/empty
> reply. A cargo `runner` (`.cargo/sign-and-run.sh`, wired up in
> `.cargo/config.toml`) re-signs the server binary on every `cargo run` — the
> same thing the .NET SDK does to its output — so the firewall allows it.
> (`build.rs` can't do this: it runs *before* the binary is linked.)
>
> A stale *block* entry from a run before this was in place overrides the
> signature; if so, clear it once:
>
> ```sh
> sudo /usr/libexec/ApplicationFirewall/socketfilterfw \
>   --unblockapp "$PWD/../target/debug/hygg-server"
> ```
>
> `docker compose` avoids the issue entirely — Docker's networking is already
> allowed through the firewall.

## Configuration

All settings come from environment variables (autoloaded from `.env`). See
[`.env.example`](.env.example) for the full list. The essentials:

| Variable | Default | Purpose |
|---|---|---|
| `HYGG_DATA_DIR` | `hygg-data` | directory the server owns and stores everything under; `DATABASE_URL` and `LOG_DIR` default beneath it |
| `DATABASE_URL` | `sqlite://hygg-data/hygg-server.db` | Path to the SQLite file |
| `LOG_DIR` | `hygg-data/hygg-logs` | base dir for rotating logs (`<LOG_DIR>/hygg-server/`, daily, 30-day retention) |
| `PORT` | `3032` | listen port (change this to move the server) |
| `HOST` | `0.0.0.0` | bind interface (`0.0.0.0` = LAN, `127.0.0.1` = local only) |
| `BIND_ADDR` | — | full `host:port` that overrides `HOST`/`PORT` |
| `MAX_BODY_BYTES` | `134217728` | max request body (caps blob uploads; 413 over) |
| `ADMIN_BOOTSTRAP_EMAIL` / `_PASSWORD` | — | create the first admin on an empty DB |
| `SESSION_SECRET` | — | reserved for signed-cookie deployments; sessions are DB-backed opaque ids |
| `RP_ID` / `RP_ORIGIN` / `RP_NAME` | localhost | WebAuthn relying party for the passkey ceremonies |

If the server was started once before `ADMIN_BOOTSTRAP_*` was set, add those
values to `.env` and restart it. Startup will create the admin, or promote and
repair an existing user with that email.

## CLI quick start

1. Start `hygg-server` and open the web UI, for example
   <http://localhost:3032/>.
2. Sign up or log in. On self-hosted installs every signup is a full user —
   there are no plans or tiers to assign.
3. Open **Devices**.
4. Create one device token per hygg client. Copy the token when it is shown; it
   is only displayed once.
5. Open hygg and run:

   ```text
   :connect http://localhost:3032
   :auth <paste-copied-device-token>
   :sync
   ```

`SERVER_URL`, `API_TOKEN`, `AUTO_SYNC`, and `DEVICE_ID` are saved to
`~/.config/hygg/.env` and reload next session. Automatic sync is enabled by
default after authentication; use `:autosync off` only when you want to opt out.

Use `:sync` to force a push/pull. Use `:server-progress` to jump to the latest
position from another device, or `:local-progress` to keep the current local
position and overwrite the server. `:disconnect` removes the server URL and
token from the CLI config, but local documents, progress, highlights, bookmarks,
and notes remain available offline.

You can also create a device token directly through the API:

```sh
curl -X POST http://localhost:3032/api/v1/devices/register \
  -H 'Content-Type: application/json' \
  -d '{"email":"you@example.com","password":"…","device_name":"laptop"}'
# -> { "device_id": "…", "token": "prefix.secret", … }   (shown once)
```

## API (v1)

All endpoints require `Authorization: Bearer <token>` except `register`.

All endpoints below `register` require a token, and nothing else is restricted.
A downstream `Entitlements` implementation may additionally answer 403 on sync,
document and blob endpoints — in its own words, which clients show as-is —
while `me` and device management stay available.

| Method & path | Purpose |
|---|---|
| `POST /api/v1/devices/register` | exchange user credentials for a device token |
| `GET /api/v1/me` | the authenticated principal |
| `GET /api/v1/devices` | list the caller's devices |
| `DELETE /api/v1/devices/{id}` | revoke a device and its tokens |
| `POST /api/v1/sync/push` | batch of ops (idempotent via `op_id`, last-write-wins) |
| `GET /api/v1/sync/pull?since=<ms>` | progress/bookmarks/highlights/notes since a cursor |
| `GET /api/v1/events` | SSE stream of `changed` notifications (push; pull to fetch) |
| `GET /api/v1/books` | list the caller's documents |
| `POST /api/v1/books` | register/update document metadata |
| `PUT/GET /api/v1/books/{hash}/blob` | upload/download document bytes |
| `GET /api/v1/export` | the caller's full personal library as a portable bundle |
| `POST /api/v1/import` | merge a bundle back into the caller's account |

`export`/`import` are the migration path between any two deployments (either
direction): the bundle carries document metadata + bytes + tags + reading
position + bookmarks/highlights/notes, and deliberately excludes machine-bound
device tokens and any deployment-specific state, so it round-trips cleanly.

`push` accepts a batch of self-describing ops; `kind` is one of `progress`,
`bookmark`, `highlight` or `note`. Deletions set `"deleted": true` (a tombstone
that propagates to other devices). Examples:

```json
{ "ops": [
  { "op_id": "<uuid>", "kind": "progress", "book_id": "<sha256>",
    "updated_at": 1719000000000,
    "data": { "offset": 250, "total_lines": 1000, "percentage": 25.0,
              "viewport_offset": 200, "cursor_y": 15 } },
  { "op_id": "<uuid>", "kind": "bookmark", "book_id": "<sha256>",
    "updated_at": 1719000000001, "data": { "mark": "a", "line": 42, "col": 0 } },
  { "op_id": "<uuid>", "kind": "highlight", "book_id": "<sha256>",
    "updated_at": 1719000000002, "data": { "start_offset": 100, "end_offset": 220 } },
  { "op_id": "<uuid>", "kind": "note", "book_id": "<sha256>",
    "updated_at": 1719000000003,
    "data": { "id": "<note-uuid>", "body": "…", "line": 7 } }
] }
```

## Security

- Per-device API tokens: 256-bit random `secret`, only `sha256(secret)` stored,
  verified in constant time. Tokens can be revoked/expired per device.
- Passwords and recovery codes use Argon2id.
- Every domain row is tenant-scoped; repository functions require a `tenant_id`.
- Devices can be restricted (read-only, per-document scope, progress-sync denied).
- All SQL is parameter-bound; internal/DB errors are never leaked to clients.

## Use as a library (extend it)

`hygg-server` is source-available (Elastic License 2.0) and published to
crates.io, and it is designed to be embedded as a Rust library and extended
in-process — no forking, no REST shim. A downstream crate can add its own rules
and pages this way, and the result is a single statically-linked binary.

The extension seams are plain Rust APIs:

- **`ext::Entitlements`** — an `Arc<dyn Entitlements>` held in `AppState`
  (default `NoopEntitlements` = fully open, nothing restricted). Override
  `resolve` (personal sync + workspace access), `sync_denial`,
  `authorize_device_registration`, `authorize_upload`, `org_caps`,
  `authorize_share_participant`, `share_limit`, `account_label`,
  `storage_limit` or `org_limits` to answer any of those questions differently.
  An override that declines supplies its own wording via `Denial`, which the
  core relays untouched — no policy or vocabulary is the core's concern.
  Install with `AppState::new(db, config).with_entitlements(...)`.
- **`ext::WebExt`** — injects presentation into the core's server-rendered
  pages: extra admin sidenav links and nav groups, extra CSS, account rows, the
  devices-page panel head, org panels, org-wizard fields (+ `on_org_created`),
  admin dashboard panels, and the redirect target for users without workspace
  access. Install with `.with_web_ext(...)`.
- **`migration::SchemaExt`** — your own migrations, run after the core's
  against the same database. Additive only: add tables that reference the
  core's by id rather than altering them. Name them so they cannot collide
  with the core's in the shared ledger. Install with `.with_schema_ext(...)`.
- **Router composition** — `routes(state)` returns the core router (state-erased,
  without `/`); `merge` your own routes onto it, then apply `layers(router, cfg)`
  and serve with `runtime::serve_router(state, router)` — or call
  `runtime::prepare` (migrate + bootstrap) and `runtime::bind_and_serve`
  separately to seed your own state in between. Your pages render through the
  public web toolkit (`web::page`, `web::esc`, `web::WebUser`,
  `web::current_user`, …).

```rust
let state = AppState::new(db, config.clone())
    .with_entitlements(Arc::new(MyEntitlements::new()))
    .with_web_ext(Arc::new(MyWebExt::new()))
    .with_schema_ext(Arc::new(SchemaExt::new(my_migrations)));
let router = hygg_server::layers(
    hygg_server::routes(state.clone()).merge(my_router().with_state(state.clone())),
    &config,
);
hygg_server::runtime::serve_router(state, router).await?;
```

## Development

```sh
cargo test -p hygg-server                 # unit + in-process API tests (SQLite)
cargo run -p hygg-server                  # uses ./.env or ./packages/hygg-server/.env
```

`hygg-server` is published to crates.io (Elastic License 2.0) but is **not** in
the `hygg` CLI's dependency tree, so `cargo install hygg` never pulls its
async/server stack — isolation is by dependency direction, not a publish flag.
