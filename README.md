# Seed Recovery Tool

A local, offline, open-source tool for recovering **partial or misremembered**
BIP39 seed phrases — a typo, a missing word, an unclear order.

**This never asks for your full phrase over a network, never phones home,
and never asks you to trust anyone with your keys.** Read the code. Run it
offline. Verify the claims yourself — don't take our word for it.

## Download

**[Get the latest release](https://github.com/Miztahj866/Seed-Recovery/releases/latest)**:
installers for Windows, macOS (Intel and Apple Silicon) and Linux, with
SHA-256 checksums to check before running. They're built by GitHub Actions
straight from this repository ([`release.yml`](.github/workflows/release.yml)).

The installers aren't code-signed yet, so Windows and macOS will warn about
an unknown publisher the first time. The release notes explain how to open
it anyway, and how to check the file first.

## What this can do

- ✅ Fix typos (fuzzy-matches misspelled words against the official BIP39 wordlist)
- ✅ Brute-force 1–3 missing/forgotten words (using the BIP39 checksum to prune invalid guesses)
- ✅ Search word order when you know the set of words but not the sequence (a full 12-word phrase takes minutes; longer phrases can take days)
- ✅ Check candidates against a Bitcoin address you know is yours (desktop app), so you get *your* phrase instead of a list of phrases that merely pass the checksum. Supports Bitcoin addresses starting with `1`, `3`, `bc1q` and `bc1p`, and Ethereum (`0x…`) addresses, including Ledger Live accounts.

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
- **Desktop GUI** (`gui/`, Tauri + Rust) — a Rust port of the same BIP39 logic,
  with its own copy of the official test vectors, a multi-core search with
  progress and cancel, and address checking. The full test suite (including
  BIP84/BIP86 reference addresses) runs in CI on Linux, macOS and Windows.
  Includes a free tier (typo fix, 1 missing word with address check, word
  order) and a paid tier (multi-word search) gated by an Ed25519-signed
  license key — see `gui/BUILD.md` and `docs/LICENSING.md`.

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

## Free vs. paid

Everything in this repo is free and open source under the MIT license,
including the CLI above.

The **desktop app** (`gui/`) has two tiers:

| | Free | Multi-word license |
|---|---|---|
| Fix typos | ✅ | ✅ |
| Find 1 missing word | ✅ | ✅ |
| Word-order search | ✅ | ✅ |
| Check against your Bitcoin or Ethereum address | ✅ | ✅ |
| Find 2+ missing words | — | ✅ |
| Price | $0 | $25 one-time |

**[Get a multi-word license on Gumroad](https://unothatguy.gumroad.com/l/jifhlt)**

The license pays for the search, not a guaranteed result. If too many words
are missing, no tool can find them; see [What this CANNOT do](#what-this-cannot-do).
License keys are Ed25519-signed and checked offline; the app never contacts
a server to validate them (details in [`docs/LICENSING.md`](docs/LICENSING.md)).

### Don't need recovery yet? Avoid needing it.

**[Don't Lose Your Crypto](https://unothatguy.gumroad.com/l/86601)** ($19, PDF)
is a 13-page plain-English guide to backing up your seed phrase: paper vs.
metal, where to store it, how to test your backup safely, and how to make
sure your family can get to your crypto if something happens to you.

Buying either one directly funds work on this tool.

## License

MIT License — see [`LICENSE`](LICENSE). Permissive by design: anyone can read,
run, fork, and build on this code, which is the whole point given how much
this project's trustworthiness depends on being open to scrutiny.


## This is not financial or legal advice

This tool is provided as-is with no warranty. You are responsible for verifying
any recovered phrase before trusting it with real funds.
