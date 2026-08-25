# Data Security

This page describes the technical measures hygg uses to protect your data, and
what each measure does and does not cover.

## Data at rest on your device

Your documents and reading data are stored locally (the CLI under your config
directory; the PWA in the browser's IndexedDB / local storage). This is device
storage — protect it with your operating system's account and disk encryption,
as you would any local file. hygg does not separately encrypt on-device storage;
end-to-end encryption protects the copies that leave your device.

## Data in transit

All sync traffic uses whatever transport your server is served over; the hosted
service and any correctly configured deployment use HTTPS/TLS, so data in
transit is encrypted between your device and the server.

## Data at rest on the server

**Without end-to-end encryption**, the sync server stores your document files
and note text so it can serve them to your other devices, run server-side
extraction, and render the web reader. Anyone with access to the server's
database can read them.

**With end-to-end encryption** (see [End-to-end encryption](encryption.md)),
document files and note text are stored as ciphertext only:

- **Cipher:** XChaCha20-Poly1305 (authenticated encryption; tampering is
  detected, not just secrecy preserved).
- **Key derivation:** Argon2id over your account passphrase with a per-account
  salt, producing a 256-bit key.
- **Key custody:** the key is derived and held **only on your devices**. The
  server never receives it and stores no key material — only a public marker
  (enabled flag, salt, and a verifier that lets a new device confirm a typed
  passphrase).
- **Enforcement:** once encryption is enabled for an account, the server
  **rejects any upload that is not a valid encrypted envelope**. A client that
  has not been set up cannot push readable bytes, so encryption cannot be
  partially on.

Under end-to-end encryption, an attacker who fully compromises the server —
including the operator — obtains only ciphertext, salts, and verifiers, none of
which reveal document or note content without your key.

## Authentication and devices

Accounts authenticate with per-device bearer tokens bound to a device machine
id; a token cannot be reused from another machine. Passwords, when used, are
exchanged once to mint a token and are not stored.

## What end-to-end encryption does not cover

- **Metadata.** Titles, authors, file sizes, and reading positions remain
  readable to the server by design, so the library and progress sync keep
  working. Do not put secrets in a document title.
- **Your own devices.** A device set up with the key can read your data; protect
  your devices and your password manager accordingly.
- **Lost keys.** Because the server holds no key, a lost key means unrecoverable
  data. There is no backdoor.

## Responsible disclosure

If you believe you have found a security issue in hygg, please report it through
the project's repository rather than a public issue, so it can be addressed
before disclosure.
