# End-to-end encryption

hygg can encrypt your documents and notes end-to-end. When it is on, every
client seals your files **on your device** before they reach the sync server,
and the server stores only unreadable ciphertext. Nobody who can see the
server — not an attacker who steals the database, not the operator of a hosted
instance — can read your documents or notes. Only a device that holds your key
can.

Encryption is **optional** and **off by default**. Turning it on is a
deliberate choice you make once per account, and then set up on each device.

> **The most important thing to know:** your key is the only way to read your
> data. If you lose it, your encrypted documents are gone — there is no
> recovery, no reset, and no operator who can help. This is what "end-to-end"
> means. **Save your key in a password manager the moment you generate it.**

## What is protected

| Data | Encrypted? |
| --- | --- |
| Document files (the bytes you import) | **Yes** — sealed before upload |
| Note text | **Yes** — sealed before upload |
| Highlighted / bookmarked text | **Yes** — it lives inside the encrypted file; only positions are sent |
| Titles and authors | No — kept readable so the library list still works |
| Reading progress, reading time | No — positions and counts, not content |

The server keeps titles so its web library and dashboards remain usable, but it
can never open a document or read a note.

## How it works

- One **account passphrase** (a strong key hygg generates for you, or one you
  choose) is entered on every client.
- Each client stretches it with **Argon2id** and a per-account salt into a
  256-bit key, then seals content with **XChaCha20-Poly1305** (authenticated
  encryption). The key never leaves your devices.
- The server stores a small public **marker**: that encryption is on, the salt,
  and a *verifier* (a value that lets a new device confirm you typed the right
  key). None of it can decrypt anything.
- Once encryption is on, the server **rejects any upload that is not encrypted.**
  A device that has not been set up literally cannot push readable bytes — this
  is why every device must be set up, and why there is no "half-on" state.

## Turn it on (first device)

### hygg CLI / TUI

1. Connect and authenticate as usual (`:connect <url>`, then
   `:auth <username> <token>`).
2. Run `:encryption setup`.
3. hygg generates a key, turns encryption on for your account, and shows the
   key on screen. **Copy it into your password manager now.** It is also saved
   to `~/.config/hygg/.env` on this device.
4. Run `:encryption convert` to seal any documents you uploaded before turning
   encryption on.

### hygg PWA (browser)

1. Open **Settings → Encryption**.
2. Tap **Turn on end-to-end encryption**.
3. The generated key appears. **Save it in your password manager.** In a
   browser the key is stored in the browser's local storage (there is no
   environment variable), so treat that browser profile as sensitive.
4. Tap **Encrypt earlier uploads** to seal documents you saved before.

## Add another device

Because uploads must be encrypted, a new device sees a wizard as soon as it
connects to an account that already uses encryption.

- **CLI / TUI:** after `:auth`, hygg tells you the account is encrypted. Run
  `:encryption use <key>`, pasting the key from your password manager. hygg
  checks it against the account verifier and refuses a wrong key rather than
  silently producing garbage. You can also set the key without the prompt:

  ```sh
  export HYGG_ENCRYPTION_KEY="your-key-here"
  ```

  Put that line in your shell profile (or your OS keychain) so it is present on
  every launch. `HYGG_ENCRYPTION_KEY` always takes precedence over the config
  file.

- **PWA:** open **Settings → Encryption**; it shows "this browser doesn't have
  the key yet". Paste the key and tap **Use this key**.

## Where the key lives

| Client | Recommended | Also possible |
| --- | --- | --- |
| CLI / TUI | `HYGG_ENCRYPTION_KEY` environment variable | `~/.config/hygg/.env` (written by the wizard) |
| PWA | Browser local storage (unavoidable — no env vars in a browser) | — |

In all cases: **the canonical copy belongs in your password manager.** The
copies on your devices are conveniences; the password manager is your backup.

## Converting existing documents

Documents uploaded before you turned encryption on are still plaintext on the
server until you convert them:

- **CLI:** `:encryption convert`
- **PWA:** Settings → Encryption → **Encrypt earlier uploads**

Conversion re-uploads each document sealed. Already-encrypted documents are
skipped, so it is safe to run more than once.

## Trade-offs to accept

- **Lose the key, lose the data.** There is no recovery path, by design.
- **No server-side extraction for encrypted documents.** The server can't OCR a
  scanned PDF or run pandoc on a DOCX it can't read. Formats that need that
  extraction must be imported on a client that can do it locally (the desktop
  or CLI client). A browser-only client will say so rather than send your file
  to the server in the clear.
- **No web reader for encrypted documents.** The server's in-browser reader
  can't render what it can't decrypt; its library still lists titles.

## Turning it off

Encryption protects an account. To stop using it on **one device** without
touching the account, use `:encryption forget` (CLI) or **Forget key on this
browser** (PWA); the device keeps working for everything except encrypted
documents until you re-add the key. There is deliberately no one-click
"decrypt my whole account" — it would require re-uploading every document in
the clear. If you need that, contact your server operator.
