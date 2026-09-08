# Code Signing Setup

Unsigned binaries are a credibility problem for this project specifically —
"unrecognized publisher" warnings are exactly what a scam recovery tool would
also trigger, so signing matters more here than for a typical hobby app.

## Do this AFTER private beta feedback, not before

Signing certificates cost real money and take time to issue. Per the roadmap,
get functional/security feedback from a small trusted group first — no point
paying for signing on a build that's about to change based on that feedback.

## Windows: Authenticode

### Certificate options, cheapest to most trusted
1. **Standard OV (Organization Validation) code signing cert** — the baseline.
   Works, but new/unknown publishers still accumulate negative SmartScreen
   reputation slowly with each download until enough people have run it
   without issue.
2. **EV (Extended Validation) cert** — more expensive and requires more identity
   verification (often a hardware token or cloud HSM), but gets **immediate**
   SmartScreen reputation instead of building it up over time. Worth it once
   you're distributing beyond a small trusted beta group.

### Where to get one
Common issuers: DigiCert, Sectigo, SSL.com. Prices and exact requirements
change, so check current offerings directly rather than relying on cached
pricing — this is exactly the kind of "current state of a specific paid
product" question worth verifying at the time you're ready to buy.

### What you'll need to provide
- Proof of legal identity/business (varies by cert type — individual vs
  organization validation have different document requirements)
- For EV: typically a hardware security key (e.g., a USB HSM) or a cloud HSM
  subscription, since EV private keys can't be stored as a plain file

### How signing plugs into this project
Tauri's build config supports specifying a signing identity. Once you have a
cert, this gets configured in `src-tauri/tauri.conf.json` under
`tauri.bundle.windows.certificateThumbprint` (or via environment variables in
CI) — consult Tauri's current Windows signing documentation when you're at
this step, since exact config keys can change between Tauri versions.

## macOS: Developer ID + Notarization

### What you need
1. An **Apple Developer Program membership** (paid, annual) — required even
   for Developer ID signing outside the App Store.
2. A **Developer ID Application certificate**, generated through your Apple
   Developer account.
3. **Notarization** — after signing, you submit the built app to Apple's
   notary service, which scans it and returns a ticket that gets "stapled" to
   your app. Without this, Gatekeeper still blocks the app on modern macOS
   even if it's signed.

### How this plugs into Tauri
Tauri has built-in support for macOS signing and notarization via environment
variables during `tauri build` (typically `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, and notarization
credentials). Check Tauri's current macOS signing guide at build time, since
Apple's own notarization tooling has changed more than once.

## Linux: no centralized gatekeeping, but still do this

- **GPG-sign every release** — sign the actual release tarball/AppImage, not
  just the checksum file.
- Publish your public GPG key somewhere with an independent trust trail: a
  long-standing GitHub account, a Keybase proof, or a keyserver — not only on
  your own website (if that gets compromised, a same-site key is worthless).

## The trust-building sequence, restated

1. Private beta (small group, informal review) — you're about to start this
2. Public repo + community scrutiny (no signed binaries yet — source-build only)
3. Narrow-scope paid security review once there's some traction
4. THEN code signing + first signed public binary release
5. Bug bounty running in parallel throughout
6. Full formal audit once there's revenue to fund one

Signing early doesn't fix the trust problem — a professionally signed scam is
still a scam. Signing matters most as a rubber stamp on something that's
already earned scrutiny, not as a substitute for it.
