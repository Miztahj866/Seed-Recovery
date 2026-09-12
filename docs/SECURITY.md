# Security Policy

## Reporting a vulnerability

If you find a security issue — anything from a checksum bug to a way this tool
could leak data over the network — please open an issue on the repository, or
(for anything sensitive) reach out privately first so we can fix it before public
disclosure. We will credit reporters publicly unless they ask not to be.

There is no bug bounty yet. There will be one once this project has any revenue —
see the roadmap in the README.

## What "secure" means for this project, specifically

Because this tool handles seed phrase fragments, the security bar is not just
"no bugs" — it's "verifiably does only what it claims, and nothing else." That means:

1. **No network calls, ever, in the recovery path.** `src/bip39.py` and
   `src/recover.py` import only Python's standard library. If you find a network
   call anywhere in these files, that is a critical severity issue.
2. **No telemetry, no analytics, no crash reporting that transmits data.**
3. **Deterministic, auditable checksum logic** — matched exactly against the
   official BIP39 reference test vectors (see `tests/test_vectors.py`).

## Current audit status (be honest about this — don't overclaim)

- ✅ Core BIP39 logic validated against official Trezor/BIP39 reference test
  vectors covering both 12-word and 24-word phrases (checksum validation,
  entropy↔mnemonic conversion, and PBKDF2-HMAC-SHA512 seed derivation) — in
  BOTH the Python reference implementation and the independent Rust port.
- ✅ CI (`.github/workflows/test.yml`) runs the full test suite on Linux,
  macOS, and Windows on every push — this is real, verified cross-platform
  proof, not just "should work."
- ⚠️ **Partial memory safety.** The Rust core wraps the derived entropy and
  seed (the actual key material) in `zeroize::Zeroizing`, so those bytes are
  overwritten with zeros in memory as soon as the caller drops them, rather
  than lingering on the heap indefinitely. This does NOT yet extend to every
  intermediate value — individual mnemonic word `String`s and the
  PBKDF2 input strings are not currently zeroized, which would require a
  larger refactor (wrapping strings throughout, or moving to a `SecretString`
  type end-to-end). Treat this as a real, open gap, not a solved problem.
- ❌ **The Python reference implementation has NO memory zeroization at all.**
  CPython's string/bytes objects are immutable and its memory model makes
  reliably zeroing them on disposal impractical without a C extension, which
  this project intentionally hasn't added, to keep the reference
  implementation dependency-free and readable in a few minutes. **The Rust/
  Tauri GUI is the safer choice for anything involving real, sensitive
  phrases** — the CLI is best treated as a spec-correctness reference and
  quick sanity-checker, not the hardened path.
- ❌ **No independent third-party security audit has been performed yet.**
  This is a solo/early-stage project. Do not treat the absence of a public
  audit report as evidence of safety — treat it as an open item.
- ✅ Reproducible build process documented (`gui/BUILD.md`) — lock files
  committed, deterministic release profile settings in place.
- ❌ No code signing yet for distributed binaries (planned before any public
  binary release — running from source is the only verifiable option until then).

## If you're testing this with a REAL seed phrase (not a test vector)

Don't, on your everyday machine, even though the tool itself is offline —
your OS, browser extensions, clipboard managers, and other running software
are all things this tool can't control or vouch for. If you need to test
recovery against a phrase that actually matters:

- Boot a live, air-gapped OS from USB (e.g., [Tails](https://tails.net/))
  with networking physically disabled, run the tool there, and never let
  the machine touch a network for the session.
- Prefer the compiled Rust/Tauri GUI over the Python CLI for anything
  beyond testing, per the memory-safety note above.
- Wipe/reformat the USB and reboot to your normal OS afterward as routine
  hygiene, not because anything specific is expected to go wrong.

## Test vectors you can safely use to try this tool right now

These are official, public BIP39 test vectors — not anyone's real funds.
Use them to see how the tool behaves before trusting it with anything else:

- `abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about`
  (valid 12-word phrase — try breaking one word to see it get flagged, or
  blank one out with `?` to see the missing-word search find it again)

## What to verify yourself before trusting this with a real seed phrase

- Read `src/bip39.py` end to end — it's short by design specifically so this is feasible.
- Run `python3 tests/test_vectors.py` and confirm it passes on your machine.
- Run the tool with your network disconnected, and confirm nothing errors out or
  hangs waiting on a connection (it shouldn't — there are no network calls to make).
- Never enter a real phrase into any version of this tool that isn't running
  locally from source you've inspected.
