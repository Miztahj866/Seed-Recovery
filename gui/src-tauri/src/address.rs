//! address.rs — check a candidate phrase against an address the user knows
//! is theirs.
//!
//! A phrase that passes the BIP39 checksum is only *valid*, not necessarily
//! *yours*: with 12 words about 1 in 16 random guesses pass. This module
//! turns each candidate into the addresses a wallet would show for it and
//! compares them with the user's address. A match proves the phrase.
//!
//! All the key math (BIP32 derivation, secp256k1, address encoding) comes
//! from the `bitcoin` crate (rust-bitcoin), which wraps libsecp256k1, the
//! elliptic-curve library used by Bitcoin Core. Nothing here is hand-rolled.
//!
//! Supported address types, and the standard path each one implies:
//!
//! | Address starts with | Type                 | Path              |
//! |---------------------|----------------------|-------------------|
//! | `1`                 | Legacy (P2PKH)       | m/44'/0'/a'/c/i   |
//! | `3`                 | Nested SegWit        | m/49'/0'/a'/c/i   |
//! | `bc1q`              | Native SegWit        | m/84'/0'/a'/c/i   |
//! | `bc1p`              | Taproot              | m/86'/0'/a'/c/i   |
//!
//! `a` = account, `c` = 0 for receiving or 1 for change, `i` = index.
//! Nothing here touches the network; it is pure computation.

use bitcoin::address::NetworkUnchecked;
use bitcoin::bip32::{ChildNumber, Xpriv, Xpub};
use bitcoin::secp256k1::{All, PublicKey, Secp256k1};
use bitcoin::{Address, AddressType, CompressedPublicKey, Network, ScriptBuf};

use crate::bip39;

/// Which kind of Bitcoin address the user gave us.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Legacy,
    NestedSegwit,
    NativeSegwit,
    Taproot,
}

impl Kind {
    /// The BIP43 "purpose" number that wallets use for this address type.
    pub fn purpose(self) -> u32 {
        match self {
            Kind::Legacy => 44,
            Kind::NestedSegwit => 49,
            Kind::NativeSegwit => 84,
            Kind::Taproot => 86,
        }
    }

    /// Plain-language name shown in the app.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Legacy => "Bitcoin legacy address (starts with 1)",
            Kind::NestedSegwit => "Bitcoin SegWit address (starts with 3)",
            Kind::NativeSegwit => "Bitcoin native SegWit address (starts with bc1q)",
            Kind::Taproot => "Bitcoin Taproot address (starts with bc1p)",
        }
    }
}

/// The user's address, decoded once before the search starts.
#[derive(Debug, Clone)]
pub struct Target {
    pub kind: Kind,
    /// The address's output script. Comparing scripts instead of address
    /// strings means nothing is re-encoded per candidate.
    script: ScriptBuf,
}

/// Decode and classify the user's address, with a readable error if it
/// can't be used.
pub fn parse_address(input: &str) -> Result<Target, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("Enter an address first.".into());
    }
    let unchecked = s.parse::<Address<NetworkUnchecked>>().map_err(|_| {
        "That doesn't look like a Bitcoin address. Check it for typos, or copy it again \
         from your wallet or an old transaction."
            .to_string()
    })?;
    let addr = unchecked.require_network(Network::Bitcoin).map_err(|_| {
        "That's a test-network address. Enter a real (mainnet) Bitcoin address.".to_string()
    })?;
    let kind = match addr.address_type() {
        Some(AddressType::P2pkh) => Kind::Legacy,
        Some(AddressType::P2sh) => Kind::NestedSegwit,
        Some(AddressType::P2wpkh) => Kind::NativeSegwit,
        Some(AddressType::P2tr) => Kind::Taproot,
        _ => {
            return Err("This kind of address (for example a multisig or script address) \
                        isn't supported yet. Use a regular single-key address from your wallet."
                .into())
        }
    };
    Ok(Target {
        kind,
        script: addr.script_pubkey(),
    })
}

/// Which addresses to try for each candidate phrase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// Accounts 0..accounts. Most people only ever use account 0.
    pub accounts: u32,
    /// Address indexes 0..addresses on each chain.
    pub addresses: u32,
    /// Also check change addresses (chain 1), not just receiving (chain 0).
    pub change: bool,
}

impl Default for Window {
    fn default() -> Self {
        Window {
            accounts: 1,
            addresses: 10,
            change: true,
        }
    }
}

impl Window {
    /// How many addresses get derived per candidate phrase.
    pub fn addresses_per_phrase(&self) -> u64 {
        let chains = if self.change { 2 } else { 1 };
        self.accounts as u64 * chains * self.addresses as u64
    }
}

/// Checks candidate phrases against one target address. Built once per
/// search and shared by every worker thread.
pub struct Checker {
    target: Target,
    window: Window,
    passphrase: String,
    secp: Secp256k1<All>,
}

impl Checker {
    pub fn new(target: Target, window: Window, passphrase: String) -> Self {
        Checker {
            target,
            window,
            passphrase,
            secp: Secp256k1::new(),
        }
    }

    pub fn window(&self) -> Window {
        self.window
    }

    /// If `words` owns the target address within the window, returns the
    /// derivation path of the matching address.
    pub fn check(&self, words: &[String]) -> Option<String> {
        // PBKDF2 with 2,048 rounds: by far the slowest step (~1 ms).
        let seed = bip39::mnemonic_to_seed(words, &self.passphrase);
        let master = Xpriv::new_master(Network::Bitcoin, &seed[..]).ok()?;
        let purpose = self.target.kind.purpose();
        let chains: &[u32] = if self.window.change { &[0, 1] } else { &[0] };

        for account in 0..self.window.accounts {
            let path = [
                ChildNumber::from_hardened_idx(purpose).ok()?,
                ChildNumber::from_hardened_idx(0).ok()?,
                ChildNumber::from_hardened_idx(account).ok()?,
            ];
            let account_xpriv = master.derive_priv(&self.secp, &path).ok()?;
            // Everything below the account level is non-hardened, so it can
            // be derived from the public key alone: faster, and no further
            // private keys are created.
            let account_xpub = Xpub::from_priv(&self.secp, &account_xpriv);

            for &chain in chains {
                let chain_xpub = account_xpub
                    .ckd_pub(&self.secp, ChildNumber::from_normal_idx(chain).ok()?)
                    .ok()?;
                for index in 0..self.window.addresses {
                    let child = chain_xpub
                        .ckd_pub(&self.secp, ChildNumber::from_normal_idx(index).ok()?)
                        .ok()?;
                    if self.script_for(&child.public_key) == self.target.script {
                        return Some(format!(
                            "m/{}'/0'/{}'/{}/{}",
                            purpose, account, chain, index
                        ));
                    }
                }
            }
        }
        None
    }

    /// The output script a wallet would use for this public key, for the
    /// target's address type.
    fn script_for(&self, pk: &PublicKey) -> ScriptBuf {
        let cpk = CompressedPublicKey(*pk);
        match self.target.kind {
            Kind::Legacy => ScriptBuf::new_p2pkh(&cpk.pubkey_hash()),
            Kind::NativeSegwit => ScriptBuf::new_p2wpkh(&cpk.wpubkey_hash()),
            Kind::NestedSegwit => {
                let redeem = ScriptBuf::new_p2wpkh(&cpk.wpubkey_hash());
                ScriptBuf::new_p2sh(&redeem.script_hash())
            }
            Kind::Taproot => {
                let (xonly, _parity) = pk.x_only_public_key();
                ScriptBuf::new_p2tr(&self.secp, xonly, None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(phrase: &str) -> Vec<String> {
        phrase.split_whitespace().map(String::from).collect()
    }

    const ABOUT: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    /// The address a wallet shows at m/purpose'/0'/account'/chain/index.
    fn address_at(words: &[String], passphrase: &str, kind: Kind, account: u32, chain: u32, index: u32) -> String {
        let checker = Checker::new(
            Target { kind, script: ScriptBuf::new() },
            Window::default(),
            passphrase.into(),
        );
        let seed = bip39::mnemonic_to_seed(words, passphrase);
        let master = Xpriv::new_master(Network::Bitcoin, &seed[..]).unwrap();
        let path = [
            ChildNumber::from_hardened_idx(kind.purpose()).unwrap(),
            ChildNumber::from_hardened_idx(0).unwrap(),
            ChildNumber::from_hardened_idx(account).unwrap(),
            ChildNumber::from_normal_idx(chain).unwrap(),
            ChildNumber::from_normal_idx(index).unwrap(),
        ];
        let key = master.derive_priv(&checker.secp, &path).unwrap();
        let pk = Xpub::from_priv(&checker.secp, &key).public_key;
        let script = checker.script_for(&pk);
        Address::from_script(&script, Network::Bitcoin).unwrap().to_string()
    }

    // Reference addresses for the standard test phrase "abandon ... about"
    // with no passphrase. BIP84 and BIP86 publish these in their test
    // vectors; the BIP44 and BIP49 ones are the widely published values
    // (e.g. Ian Coleman's BIP39 tool) for the same phrase.
    #[test]
    fn bip84_native_segwit_vectors() {
        let w = s(ABOUT);
        assert_eq!(address_at(&w, "", Kind::NativeSegwit, 0, 0, 0), "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu");
        assert_eq!(address_at(&w, "", Kind::NativeSegwit, 0, 0, 1), "bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g");
        assert_eq!(address_at(&w, "", Kind::NativeSegwit, 0, 1, 0), "bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el");
    }

    #[test]
    fn bip86_taproot_vectors() {
        let w = s(ABOUT);
        assert_eq!(address_at(&w, "", Kind::Taproot, 0, 0, 0), "bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr");
        assert_eq!(address_at(&w, "", Kind::Taproot, 0, 0, 1), "bc1p4qhjn9zdvkux4e44uhx8tc55attvtyu358kutcqkudyccelu0was9fqzwh");
        assert_eq!(address_at(&w, "", Kind::Taproot, 0, 1, 0), "bc1p3qkhfews2uk44qtvauqyr2ttdsw7svhkl9nkm9s9c3x4ax5h60wqwruhk7");
    }

    #[test]
    fn bip44_and_bip49_vectors() {
        let w = s(ABOUT);
        assert_eq!(address_at(&w, "", Kind::Legacy, 0, 0, 0), "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA");
        assert_eq!(address_at(&w, "", Kind::NestedSegwit, 0, 0, 0), "37VucYSaXLCAsxYyAPfbSi9eh4iEcbShgf");
    }

    #[test]
    fn parse_classifies_each_type() {
        let cases = [
            ("1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA", Kind::Legacy),
            ("37VucYSaXLCAsxYyAPfbSi9eh4iEcbShgf", Kind::NestedSegwit),
            ("bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu", Kind::NativeSegwit),
            ("  bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr  ", Kind::Taproot),
        ];
        for (a, k) in cases {
            assert_eq!(parse_address(a).unwrap().kind, k, "{a}");
        }
    }

    #[test]
    fn parse_rejects_bad_input() {
        assert!(parse_address("").is_err());
        assert!(parse_address("bc1qnotanaddress").is_err());
        // Testnet address.
        let e = parse_address("tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx").unwrap_err();
        assert!(e.contains("test-network"), "{e}");
    }

    #[test]
    fn checker_finds_receive_and_change_addresses() {
        let w = s(ABOUT);
        let check = |addr: &str| {
            Checker::new(parse_address(addr).unwrap(), Window::default(), String::new()).check(&w)
        };
        assert_eq!(check("bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu").as_deref(), Some("m/84'/0'/0'/0/0"));
        assert_eq!(check("bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g").as_deref(), Some("m/84'/0'/0'/0/1"));
        assert_eq!(check("bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el").as_deref(), Some("m/84'/0'/0'/1/0"));
        assert_eq!(check("bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr").as_deref(), Some("m/86'/0'/0'/0/0"));
        assert_eq!(check("1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA").as_deref(), Some("m/44'/0'/0'/0/0"));
        assert_eq!(check("37VucYSaXLCAsxYyAPfbSi9eh4iEcbShgf").as_deref(), Some("m/49'/0'/0'/0/0"));
    }

    #[test]
    fn checker_respects_the_window_and_passphrase() {
        let w = s(ABOUT);
        let target = || parse_address("bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el").unwrap(); // change 0
        let no_change = Window { change: false, ..Window::default() };
        assert_eq!(Checker::new(target(), no_change, String::new()).check(&w), None);
        // Same phrase with a passphrase is a different wallet.
        assert_eq!(Checker::new(target(), Window::default(), "TREZOR".into()).check(&w), None);
        // An address further out is only found with a wide enough window.
        let far = address_at(&w, "", Kind::NativeSegwit, 1, 0, 14);
        let narrow = Checker::new(parse_address(&far).unwrap(), Window::default(), String::new());
        assert_eq!(narrow.check(&w), None);
        let wide = Window { accounts: 2, addresses: 15, change: false };
        let wide = Checker::new(parse_address(&far).unwrap(), wide, String::new());
        assert_eq!(wide.check(&w).as_deref(), Some("m/84'/0'/1'/0/14"));
    }

    #[test]
    fn a_wrong_phrase_never_matches() {
        // Valid BIP39 phrase, but not the one that owns the address.
        let other = s("legal winner thank year wave sausage worth useful legal winner thank yellow");
        let c = Checker::new(
            parse_address("bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu").unwrap(),
            Window::default(),
            String::new(),
        );
        assert_eq!(c.check(&other), None);
    }

    #[test]
    fn end_to_end_missing_word_with_address() {
        use crate::search;
        use std::sync::atomic::AtomicBool;
        let mut w = s(ABOUT);
        w[5] = "?".into();
        let template = search::parse_template(&w).unwrap();
        let checker = Checker::new(
            parse_address("bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu").unwrap(),
            Window::default(),
            String::new(),
        );
        let cancel = AtomicBool::new(false);
        let out = search::search_missing_verified(&template, &cancel, |_| {}, |cand: &[u16]| {
            checker.check(&search::indices_to_words(cand))
        });
        let found = out.found.expect("should recover the missing word");
        assert_eq!(search::indices_to_words(&found.phrase), s(ABOUT));
        assert_eq!(found.path, "m/84'/0'/0'/0/0");
    }

    /// Timing, not correctness: run with
    /// `cargo test --release -- --ignored --nocapture timing`.
    #[test]
    #[ignore]
    fn timing_of_address_checks() {
        let w = s(ABOUT);
        let c = Checker::new(
            parse_address("bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el").unwrap(),
            Window::default(),
            "no-match".into(), // wrong passphrase, so every address gets checked
        );
        let n = 200;
        let t = std::time::Instant::now();
        for _ in 0..n {
            assert!(c.check(&w).is_none());
        }
        let per = t.elapsed().as_secs_f64() / n as f64;
        println!(
            "TIMING: {:.2} ms per phrase checked ({} addresses each, one core)",
            per * 1000.0,
            Window::default().addresses_per_phrase()
        );
    }
}
