# hygg-server

Self-hostable, multi-tenant **sync server** for the [hygg](https://github.com/kruseio/hygg)
reader. Sync your documents, reading progress, bookmarks, highlights and notes
across devices — or run hygg fully offline and never touch a server at all.

Built as a single Rust binary: `axum` + `tokio` + `sea-orm`, server-rendered
UI, no Node build step. Stores everything in **SQLite** — a file, no extra
services to run.

> Status: progress + bookmark/highlight/note sync, documents/blobs,
> per-device auth and document scopes, request body caps, payment gating, SSE push, and the
> multi-tenant data model are implemented and tested. The server-rendered web UI
> includes signup, password/recovery login, user device-token creation, and an
> admin backoffice for users, roles, disabling users, device permissions/tokens,
> recovery passwords, and passkey revocation. Full WebAuthn passkey ceremonies
> and rate limiting are still on the roadmap.

## One-command self-host (SQLite)

```sh
cd hygg-server
cp .env.example .env           # optional; sensible defaults work as-is
# set ADMIN_BOOTSTRAP_EMAIL / ADMIN_BOOTSTRAP_PASSWORD in .env to create an admin
docker compose up --build -d
```

The server listens on `http://localhost:3032`. Data (the SQLite DB and document
blobs) persists in `./data`. Open <http://localhost:3032/> in a browser for the
web UI, or check it from the shell:

```sh
curl http://localhost:3032/health      # {"status":"ok"}
```

> Use an explicit `http://` URL. Browsers may auto-upgrade a bare
> `localhost:3032` to HTTPS, which this server doesn't serve — that shows up as
> `ERR_CONNECTION_REFUSED`.

### Prebuilt image

Every release publishes a multi-arch image (`linux/amd64` + `linux/arm64`) to
GHCR, so a self-host needs no toolchain and no compile:

```sh
docker run -d -p 3032:3032 -v "$PWD/data:/app/data" \
  ghcr.io/kruseio/hygg-server:latest
```

Tags are `:latest`, `:0.1.21` (pin this) and `:0.1`. To use it from the compose
file above, replace the `build:` block with
`image: ghcr.io/kruseio/hygg-server:latest` — the volume and environment stay as
they are.

### Run from source (cargo)

```sh
cd hygg-server
cargo run -p hygg-server       # loads .env from here; data lands in ./data
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
| `DATABASE_URL` | `sqlite://data/hygg-server.db` | Path to the SQLite file |
| `LOG_DIR` | `data/logs` | base dir for rotating logs (`<LOG_DIR>/hygg-server/`, daily, 30-day retention) |
| `PORT` | `3032` | listen port (change this to move the server) |
| `HOST` | `0.0.0.0` | bind interface (`0.0.0.0` = LAN, `127.0.0.1` = local only) |
| `BIND_ADDR` | — | full `host:port` that overrides `HOST`/`PORT` |
| `MAX_BODY_BYTES` | `134217728` | max request body (caps blob uploads; 413 over) |
| `ADMIN_BOOTSTRAP_EMAIL` / `_PASSWORD` | — | create the first admin on an empty DB |
| `SESSION_SECRET` | — | reserved for signed-cookie deployments; sessions are DB-backed opaque ids |
| `RP_ID` / `RP_ORIGIN` / `RP_NAME` | localhost | WebAuthn / passkeys (future ceremonies) |

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

All endpoints below `register` require a token. Nothing else is gated: a
downstream `Entitlements` implementation may additionally answer 403 on sync,
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
in-process — no forking, no REST shim. A downstream crate can add its own rules,
pages and limits this way, and the result is a single statically-linked binary.

The extension seams are plain Rust APIs:

- **`ext::Entitlements`** — an `Arc<dyn Entitlements>` held in `AppState`
  (default `NoopEntitlements` = fully open). Override `resolve` (personal sync +
  workspace access), `authorize_device_registration`, `authorize_upload`,
  `org_caps`, `tier_label`, `storage_limit`, `org_limits` to gate access and
  drive the quota UI. Install with
  `AppState::new(db, config).with_entitlements(...)`.
- **`ext::WebExt`** — injects presentation into the core's server-rendered
  pages: extra admin sidenav links, the devices-page quota badge, org plan
  panels, org-wizard fields (+ `on_org_created`), admin dashboard panels, and
  the redirect target for users without workspace access. Install with
  `.with_web_ext(...)`.
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
cargo run -p hygg-server                  # uses ./.env or ./hygg-server/.env
```

`hygg-server` is published to crates.io (Elastic License 2.0) but is **not** in
the `hygg` CLI's dependency tree, so `cargo install hygg` never pulls its
async/server stack — isolation is by dependency direction, not a publish flag.
