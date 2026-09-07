//! bip39.rs — Dependency-minimal BIP39 implementation.
//!
//! This is a Rust port of `src/bip39.py` from the project root, which has
//! already been validated against the official Trezor/BIP39 reference test
//! vectors. The `tests` module at the bottom of this file re-runs the same
//! vectors against THIS implementation — run `cargo test` to verify it
//! yourself before trusting anything built on top of it.
//!
//! Reference: https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki

use hmac::Hmac;
use pbkdf2::pbkdf2;
use sha2::{Digest, Sha256, Sha512};
use unicode_normalization::UnicodeNormalization;

/// The official BIP39 English wordlist, embedded at compile time.
/// Embedding (rather than reading from disk at runtime) means the wordlist
/// can't be silently swapped out on a user's machine after installation.
const WORDLIST_RAW: &str = include_str!("../wordlists/english.txt");

pub fn wordlist() -> Vec<&'static str> {
    let words: Vec<&'static str> = WORDLIST_RAW.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        words.len(),
        2048,
        "Embedded wordlist has {} words, expected 2048 — do not proceed, this is corrupted.",
        words.len()
    );
    words
}

/// (word_count, entropy_bits, checksum_bits) — valid per the BIP39 spec.
fn valid_lengths(word_count: usize) -> Option<(usize, usize)> {
    match word_count {
        12 => Some((128, 4)),
        15 => Some((160, 5)),
        18 => Some((192, 6)),
        21 => Some((224, 7)),
        24 => Some((256, 8)),
        _ => None,
    }
}

fn bytes_to_bits(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:08b}", b)).collect()
}

pub fn entropy_to_checksum_bits(entropy: &[u8]) -> String {
    let checksum_len = entropy.len() * 8 / 32;
    let mut hasher = Sha256::new();
    hasher.update(entropy);
    let digest = hasher.finalize();
    let bits = bytes_to_bits(&digest);
    bits[..checksum_len].to_string()
}

pub fn entropy_to_mnemonic(entropy: &[u8]) -> Result<Vec<String>, String> {
    let entropy_bits = entropy.len() * 8;
    let valid_entropy_bits = [128, 160, 192, 224, 256];
    if !valid_entropy_bits.contains(&entropy_bits) {
        return Err(format!("Invalid entropy length: {} bits", entropy_bits));
    }

    let wl = wordlist();
    let entropy_bin = bytes_to_bits(entropy);
    let checksum_bin = entropy_to_checksum_bits(entropy);
    let full_bits = format!("{}{}", entropy_bin, checksum_bin);

    let mut words = Vec::new();
    let chars: Vec<char> = full_bits.chars().collect();
    for chunk in chars.chunks(11) {
        let chunk_str: String = chunk.iter().collect();
        let index = usize::from_str_radix(&chunk_str, 2).map_err(|e| e.to_string())?;
        words.push(wl[index].to_string());
    }
    Ok(words)
}

/// Returns (entropy_bytes, actual_checksum_bits, expected_checksum_bits).
pub fn mnemonic_to_entropy_and_checksum(
    words: &[String],
) -> Result<(Vec<u8>, String, String), String> {
    let n = words.len();
    let (entropy_bits, _checksum_bits) =
        valid_lengths(n).ok_or_else(|| format!("Invalid word count: {}", n))?;

    let wl = wordlist();
    let mut indices = Vec::with_capacity(n);
    for w in words {
        let idx = wl
            .iter()
            .position(|&x| x == w)
            .ok_or_else(|| format!("Word not in BIP39 wordlist: {:?}", w))?;
        indices.push(idx);
    }

    let full_bits: String = indices.iter().map(|i| format!("{:011b}", i)).collect();
    let entropy_bin = &full_bits[..entropy_bits];
    let actual_checksum = &full_bits[entropy_bits..];

    let entropy_bytes = bits_to_bytes(entropy_bin);
    let expected_checksum = entropy_to_checksum_bits(&entropy_bytes);

    Ok((entropy_bytes, actual_checksum.to_string(), expected_checksum))
}

fn bits_to_bytes(bits: &str) -> Vec<u8> {
    bits.as_bytes()
        .chunks(8)
        .map(|chunk| {
            let s = std::str::from_utf8(chunk).unwrap();
            u8::from_str_radix(s, 2).unwrap()
        })
        .collect()
}

pub fn is_valid_mnemonic(words: &[String]) -> bool {
    match mnemonic_to_entropy_and_checksum(words) {
        Ok((_, actual, expected)) => actual == expected,
        Err(_) => false,
    }
}

/// Derive the 64-byte BIP39 seed via PBKDF2-HMAC-SHA512 (2048 iterations),
/// per spec. Does NOT validate the checksum first — call is_valid_mnemonic()
/// beforehand if checksum validity matters for your use case.
pub fn mnemonic_to_seed(words: &[String], passphrase: &str) -> [u8; 64] {
    let mnemonic_str: String = words.join(" ").nfkd().collect();
    let salt_str: String = format!("mnemonic{}", passphrase).nfkd().collect();

    let mut seed = [0u8; 64];
    pbkdf2::<Hmac<Sha512>>(
        mnemonic_str.as_bytes(),
        salt_str.as_bytes(),
        2048,
        &mut seed,
    )
    .expect("pbkdf2 with correct output length should never fail");
    seed
}

/// Levenshtein edit distance, used for typo suggestion.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut curr = vec![0; b.len() + 1];
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = *[prev[j + 1] + 1, curr[j] + 1, prev[j] + cost]
                .iter()
                .min()
                .unwrap();
        }
        prev = curr;
    }
    prev[b.len()]
}

pub fn closest_words(word: &str, max_results: usize) -> Vec<String> {
    let wl = wordlist();
    let lower = word.to_lowercase();
    let mut scored: Vec<(&str, usize)> =
        wl.iter().map(|&w| (w, levenshtein(&lower, w))).collect();
    scored.sort_by_key(|(_, dist)| *dist);
    scored
        .into_iter()
        .take(max_results)
        .map(|(w, _)| w.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Official Trezor/BIP39 English test vectors — same set validated against
    // the Python implementation in ../../../tests/test_vectors.py.
    // Source: https://github.com/trezor/python-mnemonic/blob/master/vectors.json
    const VECTORS: &[(&str, &str, &str)] = &[
        (
            "00000000000000000000000000000000",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04",
        ),
        (
            "7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f",
            "legal winner thank year wave sausage worth useful legal winner thank yellow",
            "2e8905819b8723fe2c1d161860e5ee1830318dbf49a83bd451cfb8440c28bd6fa457fe1296106559a3c80937a1c1069be3a3a5bd381ee6260e8d9739fce1f607",
        ),
        (
            "80808080808080808080808080808080",
            "letter advice cage absurd amount doctor acoustic avoid letter advice cage above",
            "d71de856f81a8acc65e6fc851a38d4d7ec216fd0796d0a6827a3ad6ed5511a30fa280f12eb2e47ed2ac03b5c462a0358d18d69fe4f985ec81778c1b370b652a8",
        ),
        (
            "ffffffffffffffffffffffffffffffff",
            "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong",
            "ac27495480225222079d7be181583751e86f571027b0497b5b5d11218e0a8a13332572917f0f8e5a589620c6f15b11c61dee327651a14c34e18231052e48c069",
        ),
        (
            "68a79eaca2324873eacc50cb9c6eca8cc68ea5d936f98787c60c7ebc74e6ce7c",
            "hamster diagram private dutch cause delay private meat slide toddler razor book happy fancy gospel tennis maple dilemma loan word shrug inflict delay length",
            "64c87cde7e12ecf6704ab95bb1408bef047c22db4cc7491c4271d170a1b213d20b385bc1588d9c7b38f1b39d415665b8a9030c9ec653d75e65f847d8fc1fc440",
        ),
        (
            "f585c11aec520db57dd353c69554b21a89b20fb0650966fa0a9d6f74fd989d8f",
            "void come effort suffer camp survey warrior heavy shoot primary clutch crush open amazing screen patrol group space point ten exist slush involve unfold",
            "01f5bced59dec48e362f2c45b5de68b9fd6c92c6634f44d6d40aab69056506f0e35524a518034ddc1192e1dacd32c1ed3eaa3c3b131c88ed8e7e54c49a5d0998",
        ),
    ];

    fn hex_to_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn wordlist_has_2048_words() {
        assert_eq!(wordlist().len(), 2048);
    }

    #[test]
    fn official_vectors_entropy_to_mnemonic() {
        for (entropy_hex, mnemonic, _) in VECTORS {
            let entropy = hex_to_bytes(entropy_hex);
            let words = entropy_to_mnemonic(&entropy).unwrap();
            let expected: Vec<String> = mnemonic.split(' ').map(String::from).collect();
            assert_eq!(words, expected, "mismatch for entropy {}", entropy_hex);
        }
    }

    #[test]
    fn official_vectors_mnemonic_to_entropy() {
        for (entropy_hex, mnemonic, _) in VECTORS {
            let words: Vec<String> = mnemonic.split(' ').map(String::from).collect();
            let (entropy, actual_checksum, expected_checksum) =
                mnemonic_to_entropy_and_checksum(&words).unwrap();
            assert_eq!(entropy, hex_to_bytes(entropy_hex));
            assert_eq!(actual_checksum, expected_checksum);
        }
    }

    #[test]
    fn official_vectors_are_valid() {
        for (_, mnemonic, _) in VECTORS {
            let words: Vec<String> = mnemonic.split(' ').map(String::from).collect();
            assert!(is_valid_mnemonic(&words), "should be valid: {}", mnemonic);
        }
    }

    #[test]
    fn official_vectors_seed_derivation() {
        for (_, mnemonic, seed_hex) in VECTORS {
            let words: Vec<String> = mnemonic.split(' ').map(String::from).collect();
            let seed = mnemonic_to_seed(&words, "TREZOR");
            let seed_hex_actual: String = seed.iter().map(|b| format!("{:02x}", b)).collect();
            assert_eq!(&seed_hex_actual, seed_hex, "seed mismatch for {}", mnemonic);
        }
    }

    #[test]
    fn corrupted_phrase_is_rejected() {
        let mut words: Vec<String> = VECTORS[0]
            .1
            .split(' ')
            .map(String::from)
            .collect();
        let last = words.last().unwrap().clone();
        *words.last_mut().unwrap() = if last == "zoo" {
            "about".to_string()
        } else {
            "zoo".to_string()
        };
        assert!(!is_valid_mnemonic(&words));
    }

    #[test]
    fn closest_words_fixes_simple_typo() {
        let suggestions = closest_words("abandan", 3);
        assert!(suggestions.contains(&"abandon".to_string()));
    }
}
