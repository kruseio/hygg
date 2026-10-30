# Known issues

### [<-](../README.md)

Advisories this project is carrying knowingly, why they cannot be patched from
here, and what would clear each one.

`cargo audit` reports **6 vulnerabilities** against the committed lockfile —
three `zip` advisories, each reached through two transitive copies of `zip` —
and **6 warnings**: one unsoundness and five unmaintained crates. All are
inventoried below alongside related GTK3 ecosystem risks.
The audit leg in CI reports rather than gates for exactly this reason (see the
`audit` job in `.github/workflows/ci.yml`): its input is the advisory database,
which moves on its own schedule, so it is read as news rather than as a verdict
on whoever happens to have a pull request open.

None of the entries below is dismissed on GitHub. An open alert is a standing
reminder to re-check; a dismissed one is a decision nobody revisits.

---

## zip extraction advisories

|              |                                                                                    |
| ------------ | ---------------------------------------------------------------------------------- |
| **Advisory** | RUSTSEC-2026-0367 · RUSTSEC-2026-0368 · RUSTSEC-2026-0369                          |
| **Affected** | `zip` 3.0.0 (via `epub` 2.1.5) and `zip` 6.0.0 (via `ndarray-npy` 0.10.0, `tts` only) |
| **Fixed in** | `zip` 9 (9.1.3 is current)                                                         |
| **Status**   | Open — cannot be patched from this repository; not reachable from hygg's code      |
| **Assessed** | 2026-10-30                                                                         |

### What it is

All three concern extracting an archive to disk: `ZipArchive::extract` and
`ZipStreamReader::extract` preserve SUID/SGID bits, create symlinks whose
targets point outside the destination, and apply permissions through those
symlinks.

### Why it cannot be patched here

Both copies come from parents already at their latest release, and each pins a
`zip` major version below the fix:

```
cli-epub-to-text -> epub 2.1.5 -> zip 3.0.0
cli-text-reader (tts) -> ndarray-npy 0.10.0 -> zip 6.0.0
```

### What it affects

Nothing at runtime. `epub` reads chapters out of the archive into memory and
`ndarray-npy` reads the Kokoro voices `.npz` the same way; neither calls the
`extract` APIs, and no hygg crate uses `zip` directly outside tests (the
EPUB test fixtures are built with `zip` 9). The advisories clear when `epub`
and `ndarray-npy` release on `zip` 9.

---

## glib unsoundness — [Dependabot #12](https://github.com/kruseio/hygg/security/dependabot/12)

|              |                                                             |
| ------------ | ----------------------------------------------------------- |
| **Advisory** | GHSA-wrw7-89jp-8q8g · RUSTSEC-2024-0429                     |
| **Severity** | Moderate (unsound)                                          |
| **Affected** | `glib` 0.18.5 — the advisory covers `>= 0.15.0, < 0.20.0`    |
| **Fixed in** | `glib` 0.20.0 (0.22.10 is current)                          |
| **Status**   | Open, not dismissed — cannot be patched from this repository |
| **Assessed** | 2026-10-30 (first assessed 2026-08-26)                       |

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
hygg-tauri -> tauri 2.12.5 -> gtk 0.18.2 -> glib 0.18.5
                           -> wry 0.57.1 -> webkit2gtk / soup3 / javascriptcore-rs -> glib 0.18.5
```

`tauri` 2.12.5 and `wry` 0.57.1 are the latest stable releases, and they require
`gtk ^0.18` (and `webkit2gtk =2.0.2`), which in turn require `glib ^0.18`. The
`tauri` 3.0.0 alphas add optional `gtk3`/`gtk4` features, but
`tauri-runtime-wry` 3.0.0-alpha.5 still hard-requires `gtk ^0.18` and
`webkit2gtk =2.0`, and no webkitgtk-6.0 runtime has been published. `gtk` 0.19
(on `glib` 0.22) exists, but nothing in the Tauri stack uses it. There is no version of any direct dependency that
resolves `glib` to 0.20 or later, so no lockfile change and no manifest bump
reaches it. Dependabot's own automated upgrade job for this alert has failed on
every attempt, which is the same finding arrived at independently.

Backporting the one-line fix through `[patch.crates-io]` with an in-tree copy
of `glib` 0.18.5 was considered on 2026-10-30 and rejected: it means carrying a
fork of a large FFI crate for code hygg never calls, it can only be built on
Linux, and Dependabot would keep flagging the version recorded in `Cargo.lock`
regardless.

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
| **Status**   | Still in the tree; no longer reported by `cargo audit` as of 2026-10-30 — same cause and same fix as the entry above         |
| **Assessed** | 2026-10-30                                                                                                                    |

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
| RUSTSEC-2026-0319           | `anymap2` 0.13.0     | `pdf_oxide` -> `tract-onnx` -> `liquid-core`; same reach as `rustybuzz` |
| RUSTSEC-2024-0370           | `proc-macro-error` 1.0.4  | `hygg-tauri` (Linux)           |
| RUSTSEC-2024-0436           | `paste` 1.0.15       | `hygg-pwa`                          |

`rustybuzz`, `ttf-parser` and `anymap2` are the only three that reach the
published `hygg` CLI. All come from `pdf_oxide`, which is pinned for unrelated
reasons (see the comment above the `pdf_oxide` dependency in
`packages/cli-pdf-to-text/Cargo.toml`). The first two are font-shaping
libraries that are stable rather than abandoned mid-problem; `anymap2` is a
small type map used by the template engine in `tract`'s model loader.

The `unic-*` (RUSTSEC-2025-0075/0080/0081/0098/0100) and `proc-macro-error2`
(RUSTSEC-2026-0173) entries listed on 2026-08-26 cleared with the 2026-10-30
dependency upgrade.

---

## Reproducing this inventory

```sh
cargo audit                 # 6 vulnerabilities (zip), 6 warnings
./tools/ci.sh audit         # the same, as CI runs it
```

When a warning here becomes a real vulnerability, or a fix becomes reachable,
fix it on a branch of its own rather than inside whatever pull request happens
to be open — and update this page with the date it changed.
