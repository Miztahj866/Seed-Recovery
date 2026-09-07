# Reproducible Builds

Goal: anyone should be able to clone this repo, build it themselves, and get a
**byte-for-byte identical binary** to whatever we publish in Releases. That lets
an independent reviewer verify the binary we ship actually corresponds to the
public source — not just "trust us, we compiled it honestly."

This matters more for this project than most, since it handles seed phrase
fragments. Reproducibility is a mathematical trust signal, not a marketing claim.

## Why this can't be fully verified from this development sandbox

This project was scaffolded in an environment without a Rust toolchain and
without network access to install one, so the Tauri/Rust portion below has
**not yet been compiled or run** — only written and carefully mirrored against
the already-tested Python reference implementation in `../src/bip39.py`
(33/33 official test vectors passing there).

Before this GUI is trusted with a real phrase, whoever builds it locally MUST:

```bash
cd gui/src-tauri
cargo test
```

and confirm all tests in the `tests` module of `src/bip39.rs` pass — those are
the same official Trezor/BIP39 reference vectors, checked independently in Rust.
Do not skip this step. Do not trust this GUI until that command shows all tests
passing on your own machine.

## Exact build environment (pin these before publishing a real release)

Reproducibility is sensitive to every one of these — pin exact versions once
this moves toward a real release, don't leave them floating:

- **OS + version** used for the official build (e.g. `ubuntu-22.04` via CI)
- **Rust toolchain version** — pin via a `rust-toolchain.toml` file (not yet
  added; add this before the first tagged release)
- **Node version** — pin via `.nvmrc` or `package.json` engines field
- **Cargo.lock and package-lock.json** — commit both, always. Never `.gitignore`
  lock files even though `Cargo.lock` is sometimes excluded for libraries — this
  is an application, and applications should always commit their lock files so
  builds are pinned to exact dependency versions.
- **`[profile.release]` settings in `Cargo.toml`** — already set to
  `codegen-units = 1` and `lto = true` for this project, since higher
  parallelism during codegen can introduce non-determinism between builds.

## Build steps (once a Rust + Node toolchain is available)

```bash
# 1. Install exact pinned toolchain versions (see rust-toolchain.toml once added)
rustup show   # confirms the pinned version is active

# 2. Install frontend/CLI tooling
cd gui
npm ci        # NOT npm install — ci respects the lockfile exactly

# 3. Run the Rust test suite FIRST — do not proceed if this fails
cd src-tauri
cargo test

# 4. Build the release binary
cd ..
npm run build
```

The output binary will be under `gui/src-tauri/target/release/bundle/`.

## Checksum publication (once builds are real)

For every tagged release:

1. Compute `sha256sum` of every published binary/installer.
2. Publish those hashes in **two independent places** — e.g., the GitHub
   Release notes (GPG-signed tag) AND a separate pinned post (a Keybase proof,
   a long-standing forum account, etc.) that isn't controlled by the same
   infrastructure as the download itself.
3. Document, in the release notes, the exact OS/toolchain versions used, so
   an independent builder can reproduce them.
4. Encourage — don't just allow — independent verification: explicitly invite
   someone else to build from source and diff their output against the
   published hash before the release is considered "trusted."

## What's deliberately NOT done yet, and why

- **No CI pipeline configured yet.** Setting up GitHub Actions (or equivalent)
  to do the actual reproducible build is the next concrete step once this repo
  goes public — CI output is more trustworthy than a build from a single
  maintainer's laptop, since the environment is itself auditable.
- **No code signing certificates yet.** Windows Authenticode and macOS
  Developer ID signing require paid certificates; per the project roadmap,
  this comes after there's a working, tested build — not before.
- **No published binaries yet at all.** Nothing should be distributed as a
  binary until: (a) `cargo test` passes locally for a real human, (b) the repo
  is public and has had at least some outside eyes on it, per the roadmap's
  "community scrutiny before public binaries" ordering.
