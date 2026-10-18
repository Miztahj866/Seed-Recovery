# Reproducible Builds

Goal: anyone should be able to clone this repo, build it themselves, and get a
**byte-for-byte identical binary** to whatever we publish in Releases. That lets
an independent reviewer verify the binary we ship actually corresponds to the
public source — not just "trust us, we compiled it honestly."

This matters more for this project than most, since it handles seed phrase
fragments. Reproducibility is a mathematical trust signal, not a marketing claim.

## Before trusting any build

Whoever builds this locally should first run:

```bash
cd gui/src-tauri
cargo test
```

and confirm every test passes, including the official Trezor/BIP39 reference
vectors in `src/bip39.rs` and the BIP84/BIP86 reference addresses in
`src/address.rs`. The same tests gate every official release (below).

## Exact build environment (pinned)

Reproducibility is sensitive to every one of these, so each is pinned:

- **OS images** for official builds: `windows-2022`, `macos-14` (universal
  Intel + Apple Silicon build) and `ubuntu-22.04`, in
  `.github/workflows/release.yml`
- **Rust toolchain**: `rust-toolchain.toml` at the repo root. rustup installs
  it automatically the first time you build.
- **Node**: `gui/.nvmrc`
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

## Official releases

Releases are built by GitHub Actions, not on anyone's personal machine:

1. Raise the version in `gui/src-tauri/tauri.conf.json`,
   `gui/src-tauri/Cargo.toml` and `gui/package.json`, and merge to `main`.
2. Actions tab → **Release** → **Run workflow** (on `main`).
3. The workflow runs the full test suite, builds installers on all three
   systems, refuses to continue if the seller-only `gen_license` tool ended
   up in a bundle, and creates a **draft** release with the installers,
   `SHA256SUMS.txt`, release notes (from `.github/RELEASE_NOTES.md`) and the
   exact build environment.
4. Review the draft, download and try at least one installer, then press
   **Publish**. Nothing is public before that.

Pull requests that change the workflow do a dry run of the same build
without publishing anything.

Installers (`.msi`, `.exe`, `.dmg`, `.AppImage`, `.deb`) contain timestamps,
so they are not byte-for-byte reproducible even from the same source. The
app binary inside them is the part to compare.

## Checksum publication

For every tagged release:

1. Compute `sha256sum` of every published binary/installer (the release
   workflow does this and publishes `SHA256SUMS.txt`).
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

- **No code signing certificates yet.** Windows Authenticode and macOS
  Developer ID signing require paid certificates; see `docs/CODE_SIGNING.md`.
  Until then, release notes explain the "unknown publisher" warnings and how
  to check a download's checksum first. The macOS app is ad-hoc signed
  (`signingIdentity: "-"`), so macOS asks for permission to open it rather
  than reporting it as damaged.
- **No GPG-signed tags or second checksum location yet** (see the checksum
  steps above). Both are worth adding before wide distribution.
