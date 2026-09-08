# Seed Recovery Tool

A local, offline, open-source tool for recovering **partial or misremembered**
BIP39 seed phrases — a typo, a missing word, an unclear order.

**This never asks for your full phrase over a network, never phones home,
and never asks you to trust anyone with your keys.** Read the code. Run it
offline. Verify the claims yourself — don't take our word for it.

## What this can do

- ✅ Fix typos (fuzzy-matches misspelled words against the official BIP39 wordlist)
- ✅ Brute-force 1–3 missing/forgotten words (using the BIP39 checksum to prune invalid guesses)
- ✅ Search word order when you know the set of words but not the sequence (practical only for a handful of unknown positions — see warnings in the tool)

## What this CANNOT do

- ❌ Recover a fully lost phrase with zero information. There is no way to do this,
  and no legitimate tool or person can do it either. If someone claims otherwise,
  it's a scam.
- ❌ Brute-force a forgotten BIP39 passphrase (the "25th word") unless you can supply
  a short list of candidates yourself.

## Status

**Early prototype.** The core BIP39 logic in `src/bip39.py` has been validated against
the official Trezor/BIP39 reference test vectors (`tests/test_vectors.py`) — 12-word
and 24-word phrases, entropy↔mnemonic conversion, checksum validation, and PBKDF2 seed
derivation all match the published reference values exactly. See
[`docs/SECURITY.md`](docs/SECURITY.md) for what has and hasn't been independently
audited yet.

Two interfaces exist:
- **CLI** (`src/recover.py`, Python) — the original, fully tested implementation.
- **Desktop GUI** (`gui/`, Tauri + Rust) — a from-scratch Rust port of the same
  BIP39 logic, with its own copy of the official test vectors in
  `gui/src-tauri/src/bip39.rs`. **This has not yet been compiled or run** — it
  was written in an environment without a Rust toolchain available. Anyone
  building it must run `cargo test` inside `gui/src-tauri` first and confirm
  all tests pass before trusting it with a real phrase. See `gui/BUILD.md`.

## How to verify this is trustworthy, yourself

1. **Read `src/bip39.py`.** It's under 200 lines. It uses only Python's standard
   library (`hashlib`, `hmac`, `unicodedata`) — no third-party dependencies, no
   network libraries imported anywhere.
2. **Run the test vectors yourself:** `python3 tests/test_vectors.py` — this checks
   our implementation against the same reference vectors used by Go, Rust, and JS
   BIP39 libraries industry-wide.
3. **Disconnect from the internet before running anything on a real phrase.**
   The tool doesn't need connectivity to work — that's the point.
4. **Never paste a real seed phrase into any web tool, including ours if we ever
   ship a hosted version.** This should always be a local download you run yourself.

## Usage

```bash
# Fix a typo
python3 src/recover.py fix "abandan abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

# Recover one missing word (mark unknown slots with ?)
python3 src/recover.py missing "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon ?"

# Recover word order (only practical for a few unknown positions)
python3 src/recover.py reorder "about abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon"
```

Any candidate that passes the checksum is **valid BIP39**, not necessarily **your**
phrase. Always verify by deriving the resulting address and comparing it to one you
already know belongs to your wallet, before trusting a recovered phrase with funds.

## Project structure

```
seed-recovery-tool/
├── src/
│   ├── bip39.py      # Core BIP39 logic — checksum, entropy<->mnemonic, seed derivation
│   └── recover.py    # CLI: typo fix, missing-word brute force, reorder search
├── tests/
│   └── test_vectors.py   # Official Trezor/BIP39 reference test vectors (Python)
├── wordlists/
│   └── english.txt   # Official BIP39 English wordlist (2048 words)
├── docs/
│   └── SECURITY.md    # Responsible disclosure + audit status
└── gui/               # Desktop GUI (Tauri + Rust) — see gui/BUILD.md
    ├── BUILD.md        # Reproducible build process, not yet executed/verified
    ├── package.json
    ├── src/            # Plain HTML/CSS/JS frontend, no framework, no network calls
    │   ├── index.html
    │   ├── style.css
    │   └── main.js
    └── src-tauri/
        ├── Cargo.toml
        ├── tauri.conf.json   # Locked-down allowlist — no fs/shell/http APIs enabled
        ├── build.rs
        ├── wordlists/english.txt
        └── src/
            ├── bip39.rs      # Rust port of src/bip39.py, with its own official test vectors
            └── main.rs       # Tauri commands exposed to the frontend
```

## License

MIT License — see [`LICENSE`](LICENSE). Permissive by design: anyone can read,
run, fork, and build on this code, which is the whole point given how much
this project's trustworthiness depends on being open to scrutiny.

**Note:** the copyright holder name in `LICENSE` is currently a placeholder —
update it to your actual name (or entity) before any public release.

## This is not financial or legal advice

This tool is provided as-is with no warranty. You are responsible for verifying
any recovered phrase before trusting it with real funds.
