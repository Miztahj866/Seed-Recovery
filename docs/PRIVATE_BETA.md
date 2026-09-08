# Private Beta Checklist

Goal: get a small number of trusted, ideally technically-credible people to
actually run this and try to break it — before the repo goes public or any
binary gets distributed.

## Before inviting anyone

- [x] `cargo test` passes locally (done — 7/7 tests passing on Windows, including
  the official Trezor/BIP39 reference vectors)
- [x] GUI runs via `npm run dev` and all three modes (fix/missing/reorder)
  produce correct results against known test vectors
- [ ] Try the GUI on a second machine/OS if you have access to one — you've
  only confirmed Windows so far; a Mac or Linux run (even just `cargo test`
  there) would catch any Windows-specific assumptions early
- [ ] Re-read `docs/SECURITY.md` and confirm it's still accurate (no network
  calls added anywhere, no telemetry, allowlist still locked down)
- [ ] Decide what you'll say if a beta tester finds a real bug — respond fast
  and visibly; this is a trust-building opportunity, not just a fix-it task

## Who to ask

Prioritize in this order:
1. **Anyone with real security/crypto engineering background** you can reach,
   even informally — one hour of a knowledgeable friend's time reviewing
   `bip39.py` / `bip39.rs` against the BIP39 spec is worth more than ten
   non-technical testers clicking buttons.
2. **A few technically comfortable people** who can at least follow the
   "disconnect from internet, run this, tell me what happens" instructions
   and would actually try weird inputs (empty phrases, 13 words, non-English
   characters, extremely long strings).
3. Anyone who's personally dealt with a seed phrase mix-up before — they'll
   test against realistic scenarios you might not think to try.

Do NOT post publicly asking for beta testers yet. Direct, personal outreach
only at this stage — per the roadmap, public visibility comes after this step.

## What to give beta testers

- The zipped project (or a private git remote if you've pushed one) — not a
  built binary. Since nothing is code-signed yet, an unsigned binary sent
  around informally is exactly the pattern legitimate users are taught to
  distrust; having them build from source (even if that's more friction)
  reinforces the right habit from day one.
- Clear instructions: `cargo test` first, then `npm install && npm run dev`
  in `gui/`.
- Explicit permission and encouragement to try to break it — mismatched
  inputs, extremely large brute-force requests, anything adversarial.
- A way to report back (a shared doc, a private chat, whatever's easiest —
  doesn't need to be fancy for a handful of people).

## What "success" looks like for this phase

Not "no bugs found" — realistically, a few things stated in the CLI help
text or the SECURITY.md wording, or an edge case in the reorder/missing-word
input handling. Success is:
- Nothing found that touches the network or leaks phrase data anywhere
- Any real bugs found get fixed and re-tested before moving on
- You have 2-3 people willing to (informally) vouch that they tried it and
  it did what it claimed

## After private beta: what unlocks next

Once you have that — move to the public repo launch (the community post
already drafted earlier in this project), still with NO signed binaries
distributed yet, source-build only. Code signing (see
`docs/CODE_SIGNING.md`) comes after that, once there's been some public
scrutiny too.
