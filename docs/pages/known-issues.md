# Known issues

### [<-](../README.md)

Advisories this project is carrying knowingly, why they cannot be patched from
here, and what would clear each one.

`cargo audit` reports **0 vulnerabilities** against the committed lockfile. What
it does report is 21 warnings — one unsoundness and twenty unmaintained crates —
and every one of them is inventoried below. The audit leg in CI reports rather
than gates for exactly this reason (see the `audit` job in
`.github/workflows/ci.yml`): its input is the advisory database, which moves on
its own schedule, so it is read as news rather than as a verdict on whoever
happens to have a pull request open.

None of the entries below is dismissed on GitHub. An open alert is a standing
reminder to re-check; a dismissed one is a decision nobody revisits.

---

## glib unsoundness — [Dependabot #12](https://github.com/kruseio/hygg/security/dependabot/12)

|              |                                                             |
| ------------ | ----------------------------------------------------------- |
| **Advisory** | GHSA-wrw7-89jp-8q8g · RUSTSEC-2024-0429                     |
| **Severity** | Moderate (unsound)                                          |
| **Affected** | `glib` 0.18.5 — the advisory covers `>= 0.15.0, < 0.20.0`    |
| **Fixed in** | `glib` 0.20.0 (0.22.8 is current)                           |
| **Status**   | Open, not dismissed — cannot be patched from this repository |
| **Assessed** | 2026-08-26                                                   |

### What it is

`VariantStrIter::impl_get` passed `&p` — an immutable reference to a null
`*mut c_char` — to a variadic C function that writes through that pointer as an
out-argument. Recent compilers discard those writes under optimisation, so the
pointer stays null and the subsequent `CStr::from_ptr` dereferences it. The
mismatch went unnoticed because the wrapped function (`g_variant_get_child`) is
variadic, and Rust does not typecheck variadic arguments. Upstream fixed it by
passing `&mut p`.

### Why it cannot be patched here

Nothing in this repository chooses the `glib` version. It arrives through the
GTK3 stack that Tauri uses for its Linux webview:

```
hygg-tauri -> tauri 2.11.5 -> gtk 0.18.2 -> glib 0.18.5
                           -> wry -> webkit2gtk / soup3 / javascriptcore-rs -> glib 0.18.5
```

`tauri` 2.11.5 is the latest release, and it requires `gtk ^0.18`, which in turn
requires `glib ^0.18`. There is no version of any direct dependency that
resolves `glib` to 0.20 or later, so no lockfile change and no manifest bump
reaches it. Dependabot's own automated upgrade job for this alert has failed on
every attempt, which is the same finding arrived at independently.

### What it affects

`hygg-tauri` only — the native desktop/mobile shell — and only on Linux and the
BSDs, since Tauri's `gtk` dependency is gated on
`cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd",
target_os = "openbsd", target_os = "netbsd"))`. On macOS and Windows the crate
is not in the tree at all.

It reaches none of the published or shipped artefacts:

| Artefact                | Pulls `glib`? |
| ----------------------- | ------------- |
| `hygg` (the CLI on crates.io) | No      |
| `hygg-server`           | No            |
| `hygg-pwa` (wasm)       | No            |
| `hygg-tauri`            | Yes, on Linux/BSD only |

`glib` is not resolvable from the workspace's `default-members` at all —
`cargo tree -i glib` there reports no such package. Reproducing it takes an
explicit target and package:

```sh
cargo tree -p hygg-tauri --target x86_64-unknown-linux-gnu -i glib
```

The unsound function is also not one this project calls. `VariantStrIter` is
reached only through `glib`'s `Variant` iteration APIs, which the Tauri shell
does not use directly; the exposure is the transitive presence of the code, not
a call path from here.

### What clears it

Tauri moving off GTK3 — the Linux webview migrating to `webkitgtk-6.0` / GTK4,
which brings `glib` 0.20+ with it. Until then the alert stays open, and this
entry is the record of why.

---

## gtk-rs GTK3 bindings unmaintained

|              |                                                                                                                              |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| **Advisory** | RUSTSEC-2024-0411 · -0412 · -0413 · -0414 · -0415 · -0416 · -0417 · -0418 · -0419 · -0420                                    |
| **Affected** | `atk`, `atk-sys`, `gdk`, `gdk-sys`, `gdkwayland-sys`, `gdkx11`, `gdkx11-sys`, `gtk`, `gtk-sys`, `gtk3-macros` — all 0.18.2   |
| **Status**   | Open — same cause and same fix as the entry above                                                                            |
| **Assessed** | 2026-08-26                                                                                                                    |

Ten crates, one fact: the gtk-rs project no longer maintains its GTK3 bindings.
These arrive by the dependency chain above, are confined to `hygg-tauri` on
Linux/BSD, and clear when Tauri leaves GTK3 — the same event that clears the
`glib` unsoundness. They are informational: unmaintained is not a vulnerability.

---

## Other unmaintained crates

All informational, none with a known vulnerability, none actionable from here —
each is a transitive dependency whose parent chooses the version.

| Advisory                    | Crate                | Reaches                             |
| --------------------------- | -------------------- | ----------------------------------- |
| RUSTSEC-2026-0206           | `rustybuzz` 0.20.1   | `pdf_oxide` -> the CLI, server, PWA and Tauri shell |
| RUSTSEC-2026-0192           | `ttf-parser` 0.25.1  | same chain as `rustybuzz`           |
| RUSTSEC-2025-0075/0080/0081/0098/0100 | `unic-char-property`, `unic-char-range`, `unic-common`, `unic-ucd-ident`, `unic-ucd-version` — all 0.9.0 | `hygg-tauri` (Linux) |
| RUSTSEC-2024-0370           | `proc-macro-error` 1.0.4  | `hygg-tauri` (Linux)           |
| RUSTSEC-2026-0173           | `proc-macro-error2` 2.0.1 | `hygg-pwa`                     |
| RUSTSEC-2024-0436           | `paste` 1.0.15       | `hygg-pwa`                          |

`rustybuzz` and `ttf-parser` are the only two that reach the published `hygg`
CLI. Both come from `pdf_oxide`, which is pinned for unrelated reasons (see the
comment above the `pdf_oxide` dependency in
`packages/cli-pdf-to-text/Cargo.toml`), and both are font-shaping libraries
that are stable rather than abandoned mid-problem.

---

## Reproducing this inventory

```sh
cargo audit                 # 0 vulnerabilities, 21 warnings
./tools/ci.sh audit         # the same, as CI runs it
```

When a warning here becomes a real vulnerability, or a fix becomes reachable,
fix it on a branch of its own rather than inside whatever pull request happens
to be open — and update this page with the date it changed.
