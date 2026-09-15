# Payment & Fulfillment

How money actually changes hands for the paid tier, and how a license key
gets from you to the buyer, without your private signing key ever leaving
your own machine.

## The security decision this rests on

`signing_key.bin` — the private key `gen_license genkey` created — must
never touch a server. The moment it does, a server compromise means
someone can mint unlimited valid licenses, silently, forever. Every
recommendation below is built around keeping that file exactly where it
is right now: on your own computer, in `.gitignore`, nowhere else.

This means **fulfillment is a manual step you personally run**, not
something a payment platform automates end-to-end. Slower per sale, but
it's the only way to keep the "private key never leaves your machine"
property you already have.

## Recommended: Gumroad

Gumroad handles the actual payment processing and gives you a simple
product page, with zero code and zero server to maintain.

### Setup

1. Create an account at gumroad.com, add a payout method.
2. Create a new product — type "Digital product."
   - **Do NOT use Gumroad's own built-in "License key" product type.**
     Gumroad has a native license key feature that generates its own
     random key strings — these have nothing to do with the Ed25519
     signing system in `src/license.rs`. A Gumroad-native key will NOT
     pass `validate_license_key()` in the app; the app only ever accepts
     keys produced by your own `gen_license issue` command. Using
     Gumroad's feature would just confuse buyers with two unrelated
     "license keys." Pick plain "Digital product" instead.
3. Set the price (per the pricing model in `docs/LICENSING.md` — roughly
   $20-30 one-time, since you're charging for the compute/attempt, not a
   guaranteed result).
4. For the product content, don't attach an automated file — instead, set
   the product description to make clear that the license key is sent
   manually after purchase (see draft copy below). Gumroad supports manual
   fulfillment workflows; you don't need to auto-deliver anything.
5. Turn on email notifications for new sales so you see each purchase as
   it happens.

### Draft product page copy (adjust freely)

> **Seed Recovery Tool — Multi-Word Search License**
>
> Unlocks multi-word missing-word search (2+ unknown words) in the Seed
> Recovery Tool — a local, offline, open-source tool for recovering
> partial or misremembered BIP39 seed phrases. Free tier already handles
> single missing words and typo correction; this unlocks brute-force
> search across multiple unknowns.
>
> **How it works:** After purchase, you'll receive a license key by email
> within [your stated turnaround, e.g. "24 hours"]. Paste it into the
> app's Missing Word(s) tab to unlock multi-word search — no account, no
> ongoing subscription, works forever once issued.
>
> This is a one-time fee for the attempt/compute, not a guarantee of
> finding your phrase — some phrases are genuinely unrecoverable with the
> information available, and no tool (including this one) can promise
> otherwise.
>
> Source code: [link to your GitHub repo]

## Alternative: Stripe Payment Links

If you'd rather not use Gumroad's cut, a Stripe Payment Link is a
one-time setup (no code) that gives you a shareable checkout URL. You'd
handle the "please send buyers a key" step the same manual way — Stripe
just processes the payment, it doesn't know anything about license keys.

Setup is: Stripe Dashboard → Payment Links → create one for your price →
share the URL (on the GitHub repo, in your community post, wherever).
You'll get an email/dashboard notification per sale, same as Gumroad.

## Fulfillment workflow (same for either platform)

Every time you get a sale notification:

```bash
cd gui/src-tauri
cargo run --bin gen_license -- issue <buyer-order-id-or-email-hash>
```

Use whatever identifier from the sale notification makes sense — an order
number, or a hash of their email if you don't want to embed the literal
address in something that ends up in the buyer's hands. Copy the printed
license key, and send it to the buyer via whatever contact method the
payment platform gave you (Gumroad and Stripe both surface the buyer's
email after a sale).

Draft email/message to send:

> Thanks for your purchase! Here's your license key for the Seed Recovery
> Tool's multi-word search:
>
> `<paste the key here>`
>
> Paste this into the "License key" field that appears in the Missing
> Word(s) tab once you mark 2 or more words as unknown. It works offline,
> forever, no expiration — keep it somewhere safe in case you need it
> again later.
>
> If you run into any issues, just reply to this email.

## What NOT to do

- Don't put `signing_key.bin` on any server, cloud storage with API
  access, or anywhere a payment platform's automation could reach it.
- Don't build a webhook that auto-runs `gen_license issue` on a server —
  that requires the private key to live there, defeating the whole point.
- Don't reuse the same license key for multiple buyers — generate one per
  sale, using a unique identifier each time, so you retain the option to
  later investigate or handle disputes per-buyer if it ever matters.

## When automation might make sense later

If volume genuinely grows to the point where manual fulfillment is a real
bottleneck, the safer version of automation is a small self-hosted service
*you* control (not a third party's server) that holds the private key,
with real infrastructure security around it (secrets management, access
logging, the key never in plaintext at rest) — a meaningfully bigger
undertaking than anything in this project so far, and worth its own
dedicated security review before building, not something to bolt on
casually once there's revenue pressure to move fast.