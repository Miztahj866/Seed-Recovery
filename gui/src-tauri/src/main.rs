// Prevents an extra console window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bip39;
mod license;

use serde::Serialize;

const MAX_BLANKS_WITHOUT_WARNING: usize = 3;
const MAX_REORDER_WITHOUT_WARNING: usize = 6;
// Free tier: 1 missing word works regardless of license (checksum pruning
// makes this cheap). 2+ requires a valid license key, per the pricing
// model in docs/LICENSING.md. Word-order search and typo correction stay
// free at any size -- only multi-word brute force is gated.
const FREE_TIER_MAX_BLANKS: usize = 1;

#[derive(Serialize)]
struct FixResult {
    corrected_words: Vec<String>,
    suggestions: Vec<(String, Vec<String>)>, // (original_word, top matches) for words that changed
    is_valid: bool,
}

#[tauri::command]
fn fix_typos(words: Vec<String>) -> FixResult {
    let wl = bip39::wordlist();
    let mut corrected = Vec::with_capacity(words.len());
    let mut suggestions = Vec::new();

    for w in &words {
        let normalized = w.trim().to_lowercase();
        if wl.contains(&normalized.as_str()) {
            corrected.push(normalized);
        } else {
            let matches = bip39::closest_words(w, 3);
            corrected.push(matches[0].clone());
            suggestions.push((w.clone(), matches));
        }
    }

    let is_valid = bip39::is_valid_mnemonic(&corrected);
    FixResult {
        corrected_words: corrected,
        suggestions,
        is_valid,
    }
}

#[derive(Serialize)]
struct SearchResult {
    checked: u64,
    matches: Vec<Vec<String>>,
    warning: Option<String>,
}

/// words: the phrase with unknown positions marked as "?".
/// license_key: optional. Required (and must validate) if more than
/// FREE_TIER_MAX_BLANKS words are missing.
#[tauri::command]
fn search_missing(
    words: Vec<String>,
    force: bool,
    license_key: Option<String>,
) -> Result<SearchResult, String> {
    let wl = bip39::wordlist();
    let blank_positions: Vec<usize> = words
        .iter()
        .enumerate()
        .filter(|(_, w)| w.as_str() == "?")
        .map(|(i, _)| i)
        .collect();
    let n_blanks = blank_positions.len();

    if n_blanks == 0 {
        return Err("No '?' slots found — mark each unknown word position with '?'.".into());
    }

    if n_blanks > FREE_TIER_MAX_BLANKS {
        let valid = license_key
            .as_deref()
            .map(|k| license::validate_license_key(k).is_ok())
            .unwrap_or(false);
        if !valid {
            return Err(format!(
                "Searching {} missing words requires a paid license (free tier handles {}). \
                 Enter a valid license key to unlock multi-word search.",
                n_blanks, FREE_TIER_MAX_BLANKS
            ));
        }
    }

    let space = 2048u64.pow(n_blanks as u32);
    let mut warning = None;
    if n_blanks > MAX_BLANKS_WITHOUT_WARNING {
        let msg = format!(
            "{} missing words means up to {} combinations before checksum pruning. \
             This may take a very long time.",
            n_blanks, space
        );
        if !force {
            return Err(format!("{} Pass force=true to proceed anyway.", msg));
        }
        warning = Some(msg);
    }

    let mut matches = Vec::new();
    let mut checked: u64 = 0;

    // Iterative odometer-style counter over `n_blanks` positions, each 0..2048.
    let mut counters = vec![0usize; n_blanks];
    loop {
        let mut candidate = words.clone();
        for (pos, &word_idx) in blank_positions.iter().zip(counters.iter()) {
            candidate[*pos] = wl[word_idx].to_string();
        }
        checked += 1;
        if bip39::is_valid_mnemonic(&candidate) {
            matches.push(candidate);
        }

        // Increment odometer.
        let mut i = n_blanks;
        loop {
            if i == 0 {
                return Ok(SearchResult {
                    checked,
                    matches,
                    warning,
                });
            }
            i -= 1;
            counters[i] += 1;
            if counters[i] < 2048 {
                break;
            }
            counters[i] = 0;
        }
    }
}

#[tauri::command]
fn search_reorder(words: Vec<String>, force: bool) -> Result<SearchResult, String> {
    let n = words.len();
    let factorial: u64 = (1..=n as u64).product();

    let mut warning = None;
    if n > MAX_REORDER_WITHOUT_WARNING {
        let msg = format!(
            "{} words unordered means {} possible orderings — likely impractical to fully search.",
            n, factorial
        );
        if !force {
            return Err(format!("{} Pass force=true to proceed anyway.", msg));
        }
        warning = Some(msg);
    }

    let mut matches = Vec::new();
    let mut checked: u64 = 0;
    let mut indices: Vec<usize> = (0..n).collect();

    // Heap's algorithm for in-place permutation generation.
    let mut c = vec![0usize; n];
    let check = |perm: &[usize], words: &[String]| -> Vec<String> {
        perm.iter().map(|&i| words[i].clone()).collect()
    };

    let candidate = check(&indices, &words);
    checked += 1;
    if bip39::is_valid_mnemonic(&candidate) {
        matches.push(candidate);
    }

    let mut i = 0;
    while i < n {
        if c[i] < i {
            if i % 2 == 0 {
                indices.swap(0, i);
            } else {
                indices.swap(c[i], i);
            }
            let candidate = check(&indices, &words);
            checked += 1;
            if bip39::is_valid_mnemonic(&candidate) {
                matches.push(candidate);
            }
            c[i] += 1;
            i = 0;
        } else {
            c[i] = 0;
            i += 1;
        }
    }

    Ok(SearchResult {
        checked,
        matches,
        warning,
    })
}

#[derive(Serialize)]
struct LicenseStatus {
    valid: bool,
    license_id: Option<String>,
    error: Option<String>,
}

/// Checks a license key without running any search. The frontend calls this
/// when the user enters a key, to show "valid"/"invalid" feedback before
/// they attempt an actual multi-word search.
#[tauri::command]
fn validate_license(license_key: String) -> LicenseStatus {
    match license::validate_license_key(&license_key) {
        Ok(info) => LicenseStatus {
            valid: true,
            license_id: Some(info.license_id),
            error: None,
        },
        Err(e) => LicenseStatus {
            valid: false,
            license_id: None,
            error: Some(e),
        },
    }
}

#[tauri::command]
fn validate_mnemonic(words: Vec<String>) -> bool {
    bip39::is_valid_mnemonic(&words)
}

#[tauri::command]
fn derive_seed_hex(words: Vec<String>, passphrase: String) -> Result<String, String> {
    if !bip39::is_valid_mnemonic(&words) {
        return Err("Phrase does not pass BIP39 checksum — not deriving a seed from it.".into());
    }
    let seed = bip39::mnemonic_to_seed(&words, &passphrase);
    Ok(seed.iter().map(|b| format!("{:02x}", b)).collect())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            fix_typos,
            search_missing,
            search_reorder,
            validate_mnemonic,
            validate_license,
            derive_seed_hex
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
