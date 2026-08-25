# Privacy Policy

This page describes how hygg handles your data. hygg is designed so that you
stay in control of your documents — it runs fully offline by default, and any
sync is optional and self-hostable.

## The short version

- hygg works **entirely offline** with no account and no server. Nothing leaves
  your device unless you connect one.
- Sync is **opt-in**. You choose the server (the hosted service or one you run
  yourself) and what syncs, per document.
- With **end-to-end encryption** turned on, the server stores only ciphertext:
  it cannot read your documents or notes, and neither can whoever operates it.
  See [End-to-end encryption](encryption.md).

## What data exists, and where

**On your device (always):** your imported documents, reading progress,
bookmarks, highlights, and notes are stored locally so the reader works offline.

**On a sync server (only if you connect one):**

| Data | Stored | With encryption on |
| --- | --- | --- |
| Document files | Yes | **Encrypted** — server holds only ciphertext |
| Note text | Yes | **Encrypted** |
| Titles, authors, format | Yes | Readable (used for the library list) |
| Reading progress, reading time | Yes | Readable (positions and counts) |
| Account email, device tokens | Yes | Readable (needed to authenticate) |

If you never connect a server, none of the "sync server" data exists anywhere
but your own machine.

## End-to-end encryption

When you enable encryption for your account, every hygg client seals your
document files and note text **on your device** before uploading them. The
encryption key is derived from a passphrase that only you hold; it is never sent
to the server. The server keeps only a non-secret marker (that encryption is
on, a salt, and a verifier) and enforces that uploads are encrypted.

Consequences you should understand:

- The server operator **cannot** read, recover, or reset your encrypted data.
- If you lose your key, your encrypted documents are **unrecoverable**.
- The server cannot run server-side extraction (OCR, format conversion) or its
  web reader on encrypted documents, because it cannot read them.

Setup instructions are in [End-to-end encryption](encryption.md).

## Self-hosting

hygg's sync server is source-available and can be self-hosted. When you run your
own server, all sync data lives on infrastructure you control, subject to your
own policies. This document describes hygg's behavior; a hosted operator may
publish additional terms for their instance.

## Third parties

The core reader makes no third-party network calls. Optional features that do
reach the network (for example, checking the project's GitHub star count in the
PWA top bar, or a hosted deployment's subscription flow) are clearly surfaced in
the interface and can be turned off.

## Changes

This policy may change as hygg evolves. Material changes to how data is handled
will be reflected here and in the [Data Security](data-security.md) page.
