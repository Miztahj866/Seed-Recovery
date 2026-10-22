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
//! elliptic-curve library used by Bitcoin Core; Ethereum's Keccak-256 comes
//! from the RustCrypto `sha3` crate. Nothing here is hand-rolled.
//!
//! Supported address types, and the standard path each one implies:
//!
//! | Address starts with | Type                 | Path              |
//! |---------------------|----------------------|-------------------|
//! | `1`                 | Legacy (P2PKH)       | m/44'/0'/a'/c/i   |
//! | `3`                 | Nested SegWit        | m/49'/0'/a'/c/i   |
//! | `bc1q`              | Native SegWit        | m/84'/0'/a'/c/i   |
//! | `bc1p`              | Taproot              | m/86'/0'/a'/c/i   |
//! | `0x`                | Ethereum (and EVM)   | m/44'/60'/a'/0/i  |
//!
//! `a` = account, `c` = 0 for receiving or 1 for change, `i` = index.
//! MetaMask and most wallets use account 0 and count up `i`; Ledger Live
//! uses `i` = 0 and counts up the account, so the window covers both.
//! Nothing here touches the network; it is pure computation.

use bitcoin::address::NetworkUnchecked;
use bitcoin::bip32::{ChildNumber, Xpriv, Xpub};
use bitcoin::secp256k1::{All, PublicKey, Secp256k1};
use bitcoin::{Address, AddressType, CompressedPublicKey, Network, ScriptBuf};
use sha3::{Digest, Keccak256};

use crate::bip39;

/// Which kind of address the user gave us.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Legacy,
    NestedSegwit,
    NativeSegwit,
    Taproot,
    Ethereum,
}

impl Kind {
    /// The BIP43 "purpose" number that wallets use for this address type.
    pub fn purpose(self) -> u32 {
        match self {
            Kind::Legacy => 44,
            Kind::NestedSegwit => 49,
            Kind::NativeSegwit => 84,
            Kind::Taproot => 86,
            Kind::Ethereum => 44,
        }
    }

    /// The SLIP-44 coin type in the path: 0 for Bitcoin, 60 for Ethereum.
    pub fn coin_type(self) -> u32 {
        match self {
            Kind::Ethereum => 60,
            _ => 0,
        }
    }

    /// Ethereum wallets don't use change addresses.
    fn has_change(self) -> bool {
        self != Kind::Ethereum
    }

    /// Plain-language name shown in the app.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Legacy => "Bitcoin legacy address (starts with 1)",
            Kind::NestedSegwit => "Bitcoin SegWit address (starts with 3)",
            Kind::NativeSegwit => "Bitcoin native SegWit address (starts with bc1q)",
            Kind::Taproot => "Bitcoin Taproot address (starts with bc1p)",
            Kind::Ethereum => "Ethereum address (starts with 0x). Also works for other EVM networks",
        }
    }
}

/// The user's address, decoded once before the search starts. Candidates
/// are compared in this raw form, so nothing is re-encoded per candidate.
#[derive(Debug, Clone)]
pub struct Target {
    pub kind: Kind,
    expected: Expected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expected {
    /// Bitcoin: the address's output script.
    Script(ScriptBuf),
    /// Ethereum: the 20-byte address.
    Eth([u8; 20]),
}

/// Decode and classify the user's address, with a readable error if it
/// can't be used.
pub fn parse_address(input: &str) -> Result<Target, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("Enter an address first.".into());
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return parse_eth_address(hex);
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
        expected: Expected::Script(addr.script_pubkey()),
    })
}

/// `hex` is the part after "0x". Checks the EIP-55 capitalisation
/// checksum when the address uses mixed case, since a typo there is
/// otherwise invisible.
fn parse_eth_address(hex: &str) -> Result<Target, String> {
    if hex.len() != 40 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("An Ethereum address is 0x followed by 40 characters (0-9 and a-f). \
                    Check it for typos."
            .into());
    }
    let mut bytes = [0u8; 20];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|_| "Invalid address".to_string())?;
    }
    let has_upper = hex.bytes().any(|b| b.is_ascii_uppercase());
    let has_lower = hex.bytes().any(|b| b.is_ascii_lowercase());
    if has_upper && has_lower && eip55(&bytes)[2..] != *hex {
        return Err("The capital letters in this Ethereum address don't match its built-in \
                    checksum, which usually means a typo. Copy it again from your wallet or a \
                    block explorer."
            .into());
    }
    Ok(Target {
        kind: Kind::Ethereum,
        expected: Expected::Eth(bytes),
    })
}

/// The Ethereum address for a public key: the last 20 bytes of the
/// Keccak-256 hash of the uncompressed key (without its 0x04 prefix).
fn eth_address(pk: &PublicKey) -> [u8; 20] {
    let uncompressed = pk.serialize_uncompressed();
    let hash = Keccak256::digest(&uncompressed[1..]);
    let mut out = [0u8; 20];
    out.copy_from_slice(&hash[12..]);
    out
}

/// EIP-55 mixed-case form of an address, e.g. 0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed.
fn eip55(addr: &[u8; 20]) -> String {
    let lower: String = addr.iter().map(|b| format!("{:02x}", b)).collect();
    let hash = Keccak256::digest(lower.as_bytes());
    let mut out = String::from("0x");
    for (i, c) in lower.chars().enumerate() {
        let nibble = (hash[i / 2] >> if i % 2 == 0 { 4 } else { 0 }) & 0x0f;
        if c.is_ascii_alphabetic() && nibble >= 8 {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
    }
    out
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
        let kind = self.target.kind;
        let purpose = kind.purpose();
        let coin = kind.coin_type();
        let chains: &[u32] = if self.window.change && kind.has_change() {
            &[0, 1]
        } else {
            &[0]
        };

        for account in 0..self.window.accounts {
            let path = [
                ChildNumber::from_hardened_idx(purpose).ok()?,
                ChildNumber::from_hardened_idx(coin).ok()?,
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
                    if self.matches(&child.public_key) {
                        return Some(format!(
                            "m/{}'/{}'/{}'/{}/{}",
                            purpose, coin, account, chain, index
                        ));
                    }
                }
            }
        }
        None
    }

    fn matches(&self, pk: &PublicKey) -> bool {
        match &self.target.expected {
            Expected::Script(script) => self.script_for(pk) == *script,
            Expected::Eth(addr) => eth_address(pk) == *addr,
        }
    }

    /// The output script a wallet would use for this public key, for the
    /// target's (Bitcoin) address type.
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
            Kind::Ethereum => ScriptBuf::new(), // not used: Ethereum compares addresses
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

    /// The address a wallet shows at m/purpose'/coin'/account'/chain/index.
    fn address_at(words: &[String], passphrase: &str, kind: Kind, account: u32, chain: u32, index: u32) -> String {
        let checker = Checker::new(
            Target { kind, expected: Expected::Script(ScriptBuf::new()) },
            Window::default(),
            passphrase.into(),
        );
        let seed = bip39::mnemonic_to_seed(words, passphrase);
        let master = Xpriv::new_master(Network::Bitcoin, &seed[..]).unwrap();
        let path = [
            ChildNumber::from_hardened_idx(kind.purpose()).unwrap(),
            ChildNumber::from_hardened_idx(kind.coin_type()).unwrap(),
            ChildNumber::from_hardened_idx(account).unwrap(),
            ChildNumber::from_normal_idx(chain).unwrap(),
            ChildNumber::from_normal_idx(index).unwrap(),
        ];
        let key = master.derive_priv(&checker.secp, &path).unwrap();
        let pk = Xpub::from_priv(&checker.secp, &key).public_key;
        if kind == Kind::Ethereum {
            return eip55(&eth_address(&pk));
        }
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

    // --- Ethereum ---

    #[test]
    fn keccak256_reference_vector() {
        // Keccak-256 of the empty string (as used by Ethereum, not SHA3-256).
        let h = Keccak256::digest(b"");
        let hex: String = h.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(hex, "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");
    }

    #[test]
    fn eip55_reference_vectors() {
        // From the EIP-55 specification.
        for a in [
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
            "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
            "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
            "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb",
        ] {
            let t = parse_address(a).unwrap_or_else(|e| panic!("{a}: {e}"));
            assert_eq!(t.kind, Kind::Ethereum);
            match t.expected {
                Expected::Eth(bytes) => assert_eq!(eip55(&bytes), a),
                _ => panic!("not parsed as Ethereum"),
            }
        }
    }

    #[test]
    fn eth_parse_accepts_lowercase_and_rejects_typos() {
        assert!(parse_address("0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed").is_ok());
        assert!(parse_address("0X5AAEB6053F3E94C9B9A09F33669435E7EF1BEAED").is_ok());
        // One letter's case flipped: fails the EIP-55 checksum.
        let e = parse_address("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAEd").unwrap_err();
        assert!(e.contains("checksum"), "{e}");
        assert!(parse_address("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeA").is_err()); // too short
        assert!(parse_address("0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAeZ").is_err()); // not hex
    }

    #[test]
    fn eth_reference_address() {
        // The widely published first Ethereum address (m/44'/60'/0'/0/0) for
        // the standard "abandon ... about" test phrase.
        let w = s(ABOUT);
        assert_eq!(address_at(&w, "", Kind::Ethereum, 0, 0, 0), "0x9858EfFD232B4033E47d90003D41EC34EcaEda94");
    }

    #[test]
    fn checker_finds_metamask_and_ledger_live_style_addresses() {
        let w = s(ABOUT);
        let first = Checker::new(
            parse_address("0x9858EfFD232B4033E47d90003D41EC34EcaEda94").unwrap(),
            Window::default(),
            String::new(),
        );
        assert_eq!(first.check(&w).as_deref(), Some("m/44'/60'/0'/0/0"));

        // MetaMask style: account 0, a later index.
        let metamask = address_at(&w, "", Kind::Ethereum, 0, 0, 6);
        let c = Checker::new(parse_address(&metamask).unwrap(), Window::default(), String::new());
        assert_eq!(c.check(&w).as_deref(), Some("m/44'/60'/0'/0/6"));

        // Ledger Live style: index 0 of a later account; needs the accounts window.
        let ledger = address_at(&w, "", Kind::Ethereum, 3, 0, 0);
        let narrow = Checker::new(parse_address(&ledger).unwrap(), Window::default(), String::new());
        assert_eq!(narrow.check(&w), None);
        let wide = Window { accounts: 5, ..Window::default() };
        let wide = Checker::new(parse_address(&ledger).unwrap(), wide, String::new());
        assert_eq!(wide.check(&w).as_deref(), Some("m/44'/60'/3'/0/0"));
    }

    #[test]
    fn end_to_end_missing_word_with_eth_address() {
        use crate::search;
        use std::sync::atomic::AtomicBool;
        let mut w = s(ABOUT);
        w[9] = "?".into();
        let template = search::parse_template(&w).unwrap();
        let checker = Checker::new(
            parse_address("0x9858EfFD232B4033E47d90003D41EC34EcaEda94").unwrap(),
            Window::default(),
            String::new(),
        );
        let cancel = AtomicBool::new(false);
        let out = search::search_missing_verified(&template, &cancel, |_| {}, |cand: &[u16]| {
            checker.check(&search::indices_to_words(cand))
        });
        let found = out.found.expect("should recover the missing word");
        assert_eq!(search::indices_to_words(&found.phrase), s(ABOUT));
        assert_eq!(found.path, "m/44'/60'/0'/0/0");
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
