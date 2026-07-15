---
name: verify
description: Build, launch, and drive hygg-pwa in a browser to verify changes at the UI surface.
---

# Verifying hygg-pwa

Rust/WASM (Leptos CSR) app built by Trunk.

## Fast type/lint gate (not verification)

```bash
cargo check -p hygg-pwa --target wasm32-unknown-unknown
cargo clippy -p hygg-pwa --target wasm32-unknown-unknown -- -D warnings
cargo fmt -p hygg-pwa
```

## Launch

```bash
cd packages/hygg-pwa && trunk serve   # serves http://127.0.0.1:8080, run in background
# poll http://127.0.0.1:8080/ until 200 — first wasm build takes ~1–2 min
```

`cargo run -p hygg-pwa` is an equivalent launcher that shells out to trunk.

## Drive

Use the Playwright MCP tools against http://127.0.0.1:8080/.

Gotchas:
- **Never deep-link routes in dev** (e.g. `goto /settings`): `base_path()`
  derives the router base from `document.baseURI`, and dev has no `<base>`
  tag, so a deep link is misread as the deploy base and renders Home.
  Always land on `/` and click through the UI (production Pages deploys
  inject `<base href>`, so this is dev-only).
- Settings persist in `localStorage` under key `hygg.settings` — inspect it
  to confirm persistence; clear it to simulate a fresh install.
- Network calls (e.g. the GitHub stars fetch) can be counted via
  `performance.getEntriesByType('resource')`.
- Two console warnings are pre-existing boilerplate (preload integrity,
  apple-mobile-web-app-capable) — not regressions.
