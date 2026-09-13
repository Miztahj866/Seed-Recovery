# License Key System

How the paid tier is actually gated: multi-word missing-word search (2+
blanks) requires a valid license key. Everything else — typo correction,
1 missing word, word-order search — stays free at any size.

This uses Ed25519 signatures so the app only ever needs to ship your
**public** key. Nobody can mint a valid license just by inspecting the
app's source code or binary.

## One-time setup: generate your signing keypair

```bash
cd gui/src-tauri
cargo run --bin gen_license -- genkey
```

This prints something like:

```
Private key saved to: signing_key.bin
Public key (paste this into src/license.rs's PUBLIC_KEY_BYTES):

const PUBLIC_KEY_BYTES: [u8; 32] = [12, 45, 200, ...];
```

**Do this immediately after:**
1. Copy that `const PUBLIC_KEY_BYTES = [...]` line and paste it into
   `src/license.rs`, replacing the placeholder `[0u8; 32]` line.
2. Confirm `signing_key.bin` is listed in `.gitignore` (it already is,
   under `gui/src-tauri/signing_key.bin` — just don't move the file
   somewhere that isn't covered).
3. **Back up `signing_key.bin` somewhere safe and private** — a password
   manager's file storage, an encrypted drive, etc. If you lose it, you
   can't issue new licenses under this public key anymore (though every
   license you've already issued keeps working fine — validation only
   needs the public key).
4. Rebuild the app (`cargo test` and `npm run dev`) so the new public key
   is compiled in.

## Issuing a license after a sale

```bash
cd gui/src-tauri
cargo run --bin gen_license -- issue <some-identifier>
```

Use any identifier you want for `<some-identifier>` — an order number, a
timestamp, an email hash, whatever helps you keep track on your end. It's
embedded in the license (visible to whoever holds the key, so don't put
anything sensitive in it), but the app doesn't attach any special meaning
to its contents beyond "this is what got signed."

This prints the actual license key string — a long value like:

```
b3JkZXItMTIz.MEUCIQDx7z...
```

Give that whole string to the customer. They paste it into the "License
key" field that appears in the GUI once they mark 2+ words as missing.

## What the app does with it

- Below 2 missing words: works with no key needed, always.
- 2+ missing words: the GUI's license field appears, they paste the key,
  the app calls `validate_license` (a local, offline signature check — no
  network call) to show valid/invalid feedback, and passes the key along
  when they click Search.
- The Rust backend independently re-validates the key server-side... except
  there's no server. It re-validates locally in `search_missing` itself, so
  the check can't be bypassed by tricking just the UI layer — the actual
  search logic refuses to run without a valid key past the free tier,
  regardless of what the frontend shows.

## The honest limitation (state this to yourself, and to anyone reviewing this)

**No client-side check, cryptographic or not, can stop someone from patching
the compiled binary to skip the check entirely.** Ed25519 signing solves a
narrower, real problem — stopping someone from generating their *own* valid
license keys just by having the app or its source code — but it does not
and cannot stop a determined person from modifying the binary itself to
always return "valid." This is true of literally every offline license
scheme that has ever existed for any software; it's not a gap specific to
this implementation. The alternative (a server that validates on every
launch) would reintroduce the network dependency this whole project is
built to avoid, so it's a deliberate tradeoff, not an oversight.

In practice: this stops casual key-sharing and guessing. It will not stop
someone determined to reverse-engineer a decently obfuscated Rust binary.
For a tool at this price point and scale, that's a reasonable tradeoff —
revisit if the economics ever change enough to justify a different model.

## Testing this yourself without a real sale

```bash
cd gui/src-tauri
cargo run --bin gen_license -- genkey    # only if you haven't already
cargo run --bin gen_license -- issue test-license-1
```

Copy the printed key, run the app (`npm run dev` from `gui/`), enter a
phrase with 2+ `?` marks in the Missing Word(s) tab, paste the key into
the license field, and confirm it shows "✓ valid" and lets the search run.
Then try an obviously wrong key (mash the keyboard) and confirm it's
rejected.
