# Security & Review Status
For detailed audit status, known limitations, and safe test vectors to
try before trusting this with a real phrase, see
[docs/SECURITY.md](docs/SECURITY.md).

## What this tool does
This is a BIP39 seed phrase recovery tool — it helps recover wallets when
part of a seed phrase is missing or incorrect, by searching valid word
combinations against the BIP39 checksum.

## Current review status
- ✅ Functionally tested by a small group of non-security testers — recovery
  works as expected on the cases they tried.
- ❌ **Not yet reviewed by anyone with cryptography/BIP39 security
  expertise.** The derivation logic, checksum validation, and the license
  key system (Ed25519-signed, manually fulfilled) have not been
  independently audited.

## We're looking for review, specifically on:
- Correctness of BIP39 checksum/derivation logic — are there edge cases
  where the tool could return a false positive or miss a valid recovery?
- Whether the recovery process ever handles, logs, or transmits seed data
  in a way that could leak it
- Whether the license key verification could be bypassed or spoofed
- General code-level security review of the CLI and GUI

## How to report a finding
Please open an issue tagged `security`, or email me unothat1guy@gmail.com, especially for anything that could put a user's funds at risk.

As thanks, anyone who finds a legitimate security issue (recovery logic,
license verification, or data handling) will get a free lifetime license
key for the paid tier. Responsible disclosure appreciated — happy to
credit you publicly too if you'd like.

## What this tool is NOT
This tool does not guarantee recovery, does not transmit your seed phrase
anywhere, and should be run offline/air-gapped for anything involving a
real wallet. Use at your own risk with real funds.