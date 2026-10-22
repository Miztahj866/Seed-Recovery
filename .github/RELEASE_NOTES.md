Offline recovery for a BIP39 seed phrase with a missing, misspelled or out-of-order word. Your words never leave your computer.

## What's new in v__VERSION__

- **Ethereum addresses.** Paste a `0x…` address (MetaMask, Trezor, Ledger and most other wallets) and the app finds the phrase that owns it. Works for other EVM networks too, since they share the same addresses. Ledger Live users: raise "Accounts to check" under Advanced search settings.
- Typos in mixed-case Ethereum addresses are caught before the search starts.

Also in this version, from v0.2.0:

- **Check against your wallet address** (Bitcoin addresses starting with `1`, `3`, `bc1q` and `bc1p`). Free for one missing word.
- **Fast searches** on every core of your computer, with a progress bar, time left and a Cancel button.
- **Windows, macOS and Linux** installers.

## Which file do I download?

| Your computer | Download |
| --- | --- |
| Windows | `Seed.Recovery.Tool___VERSION___x64-setup.exe` (or the `.msi`) |
| Mac (Intel or Apple Silicon) | `Seed.Recovery.Tool___VERSION___universal.dmg` |
| Linux | `.AppImage` (any distro) or `.deb` (Debian, Ubuntu) |

## Before you run it

**Disconnect from the internet first** if you're entering a real phrase. The app never needs a connection.

**You'll see a security warning the first time.** The app isn't code-signed with a paid certificate yet, so your system doesn't recognise the publisher. That's expected for this release. Check the file's checksum (below) first, then:

- **Windows:** on "Windows protected your PC", click **More info**, then **Run anyway**.
- **Mac:** open the app once. When macOS says it can't verify it, go to **System Settings → Privacy & Security**, scroll down, and click **Open Anyway**.
- **Linux (AppImage):** make it runnable first: `chmod +x *.AppImage`.

## Check your download

Compare your file's SHA-256 checksum with the list below and in `SHA256SUMS.txt`. If it doesn't match exactly, don't run it: download it again from this page only.

- **Windows (PowerShell):** `Get-FileHash .\<file>`
- **Mac:** `shasum -a 256 <file>`
- **Linux:** `sha256sum <file>`

The full source code is public on this repository, and these files are built by GitHub Actions directly from it. The build environment is listed at the bottom.

**Need more than one missing word?** The multi-word license is at https://unothatguy.gumroad.com/l/jifhlt.
