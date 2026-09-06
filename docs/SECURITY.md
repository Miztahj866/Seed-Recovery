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
  entropy↔mnemonic conversion, and PBKDF2-HMAC-SHA512 seed derivation).
- ❌ **No independent third-party security audit has been performed yet.**
  This is a solo/early-stage project. Do not treat the absence of a public
  audit report as evidence of safety — treat it as an open item.
- ❌ No reproducible build process yet (planned — see README roadmap).
- ❌ No code signing yet for distributed binaries (planned before any public
  binary release — running from source is the only verifiable option until then).

## What to verify yourself before trusting this with a real seed phrase

- Read `src/bip39.py` end to end — it's short by design specifically so this is feasible.
- Run `python3 tests/test_vectors.py` and confirm it passes on your machine.
- Run the tool with your network disconnected, and confirm nothing errors out or
  hangs waiting on a connection (it shouldn't — there are no network calls to make).
- Never enter a real phrase into any version of this tool that isn't running
  locally from source you've inspected.
