// Prevents an extra console window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod address;
mod bip39;
mod license;
mod search;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

const MAX_BLANKS_WITHOUT_WARNING: usize = 3;
// With an address check, every checksum-valid phrase costs ~1-2 ms more.
// Two missing words on 12 (~262,000 checks) take minutes; three take days.
const MAX_BLANKS_WITH_ADDRESS_WITHOUT_WARNING: usize = 2;
// 12! = 479 million orderings now takes minutes with the multi-core search
// (see search.rs). 15! = 1.3 trillion would take days, so warn above 12.
const MAX_REORDER_WITHOUT_WARNING: usize = 12;
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
    /// Candidates actually checked (less than `total` if cancelled).
    checked: u64,
    /// Size of the whole search space. Sent as a float because it can
    /// exceed what JavaScript numbers hold exactly; it is only displayed.
    total: f64,
    /// How many candidates passed the checksum, including ones not listed.
    match_count: u64,
    /// Up to search::DEFAULT_MATCH_CAP matching phrases.
    matches: Vec<Vec<String>>,
    /// True when `match_count` is larger than the list in `matches`.
    truncated: bool,
    cancelled: bool,
    warning: Option<String>,
    elapsed_ms: u64,
    /// Address mode: the phrase that owns the address, if one was found.
    found: Option<FoundPhrase>,
    /// Address mode: true when an address was checked at all.
    address_checked: bool,
}

#[derive(Serialize)]
struct FoundPhrase {
    words: Vec<String>,
    path: String,
}

/// Progress update sent to the window as a "search-progress" event.
#[derive(Clone, Serialize)]
struct Progress {
    checked: u64,
    total: f64,
}

/// Shared between the running search and the cancel_search command.
/// Only one search runs at a time.
#[derive(Default)]
struct SearchControl {
    cancel: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}

/// Clears the "running" flag when a search ends, however it ends.
struct RunningGuard(Arc<AtomicBool>);

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// Runs a search on a background thread so the window stays responsive,
/// forwards throttled progress events, and turns the engine's Outcome
/// into what the frontend expects.
async fn run_search<S>(
    window: tauri::Window,
    control: &SearchControl,
    total: f64,
    warning: Option<String>,
    address_checked: bool,
    job: S,
) -> Result<SearchResult, String>
where
    S: FnOnce(&AtomicBool, &(dyn Fn(u64) + Sync)) -> search::Outcome + Send + 'static,
{
    if control.running.swap(true, Ordering::SeqCst) {
        return Err("A search is already running. Wait for it to finish or press Cancel.".into());
    }
    let guard = RunningGuard(control.running.clone());
    control.cancel.store(false, Ordering::SeqCst);
    let cancel = control.cancel.clone();

    let task = tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let started = Instant::now();
        // The Mutex makes the reporter safe to call from every search
        // thread at once; try_lock skips an update if another thread is
        // already sending one.
        let emitter = Mutex::new((started, window));
        let report = |checked: u64| {
            if let Ok(mut e) = emitter.try_lock() {
                if e.0.elapsed() >= PROGRESS_INTERVAL {
                    e.0 = Instant::now();
                    let _ = e.1.emit("search-progress", Progress { checked, total });
                }
            }
        };
        let report: &(dyn Fn(u64) + Sync) = &report;
        let outcome = job(&*cancel, report);
        (outcome, started.elapsed())
    });

    let (outcome, elapsed) = task
        .await
        .map_err(|e| format!("The search stopped unexpectedly: {}", e))?;

    Ok(SearchResult {
        checked: outcome.checked,
        total: outcome.total as f64,
        match_count: outcome.match_count,
        truncated: outcome.truncated(),
        cancelled: outcome.cancelled,
        matches: outcome
            .matches
            .iter()
            .map(|m| search::indices_to_words(m))
            .collect(),
        warning,
        elapsed_ms: elapsed.as_millis() as u64,
        address_checked,
        found: outcome.found.map(|f| FoundPhrase {
            words: search::indices_to_words(&f.phrase),
            path: f.path,
        }),
    })
}

/// Optional settings for checking candidates against a known address.
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AddressOptions {
    address: Option<String>,
    passphrase: Option<String>,
    accounts: Option<u32>,
    addresses: Option<u32>,
    change: Option<bool>,
}

/// words: the phrase with unknown positions marked as "?".
/// license_key: optional. Required (and must validate) if more than
/// FREE_TIER_MAX_BLANKS words are missing.
/// address_options: optional. With an address, only the phrase that owns
/// it is returned, instead of every phrase that passes the checksum.
#[tauri::command]
async fn search_missing(
    window: tauri::Window,
    control: tauri::State<'_, SearchControl>,
    words: Vec<String>,
    force: bool,
    license_key: Option<String>,
    address_options: Option<AddressOptions>,
) -> Result<SearchResult, String> {
    let template = search::parse_template(&words)?;
    let n_blanks = template.iter().filter(|w| w.is_none()).count();

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

    // Address check: parse everything up front so mistakes show before
    // a long search starts.
    let opts = address_options.unwrap_or_default();
    let checker = match opts.address.as_deref().map(str::trim) {
        Some(a) if !a.is_empty() => {
            let target = address::parse_address(a)?;
            let window_opts = address::Window {
                accounts: opts.accounts.unwrap_or(1).clamp(1, 20),
                addresses: opts.addresses.unwrap_or(10).clamp(1, 1000),
                change: opts.change.unwrap_or(true),
            };
            Some(address::Checker::new(
                target,
                window_opts,
                opts.passphrase.unwrap_or_default(),
            ))
        }
        _ => None,
    };

    let space = 2048u128.pow(n_blanks as u32);
    let mut warning = None;
    if let Some(c) = &checker {
        if n_blanks > MAX_BLANKS_WITH_ADDRESS_WITHOUT_WARNING {
            // About 1 in 2^(words/3) candidates pass the checksum, and each
            // of those gets the slow address check.
            let checks = space >> (words.len() / 3);
            let msg = format!(
                "{} missing words with an address check means about {} phrases to test \
                 against your address, {} addresses each. This may take days.",
                n_blanks,
                checks,
                c.window().addresses_per_phrase()
            );
            if !force {
                return Err(format!("{} Pass force=true to proceed anyway.", msg));
            }
            warning = Some(msg);
        }
    } else if n_blanks > MAX_BLANKS_WITHOUT_WARNING {
        let msg = format!(
            "{} missing words means {} combinations to check. \
             This may take a very long time.",
            n_blanks, space
        );
        if !force {
            return Err(format!("{} Pass force=true to proceed anyway.", msg));
        }
        warning = Some(msg);
    }

    let address_checked = checker.is_some();
    run_search(window, &control, space as f64, warning, address_checked, move |cancel, report| {
        match checker {
            Some(c) => search::search_missing_verified(&template, cancel, report, |cand: &[u16]| {
                c.check(&search::indices_to_words(cand))
            }),
            None => search::search_missing(&template, search::DEFAULT_MATCH_CAP, cancel, report),
        }
    })
    .await
}

#[derive(Serialize)]
struct AddressInfo {
    label: String,
}

/// Lets the window say which kind of address the user typed, or what's
/// wrong with it, before they start a search.
#[tauri::command]
fn check_address(address: String) -> Result<AddressInfo, String> {
    address::parse_address(&address).map(|t| AddressInfo {
        label: t.kind.label().to_string(),
    })
}

#[tauri::command]
async fn search_reorder(
    window: tauri::Window,
    control: tauri::State<'_, SearchControl>,
    words: Vec<String>,
    force: bool,
) -> Result<SearchResult, String> {
    let template = search::parse_template(&words)?;
    if template.iter().any(|w| w.is_none()) {
        return Err(
            "Word order search needs every word. For unknown words, use the Missing Word(s) tab."
                .into(),
        );
    }
    let indices: Vec<u16> = template.into_iter().flatten().collect();
    let n = indices.len();

    let orderings: u128 = (1..=n as u128).product();
    let mut warning = None;
    if n > MAX_REORDER_WITHOUT_WARNING {
        let msg = format!(
            "{} words unordered means {} possible orderings — likely impractical to fully search.",
            n, orderings
        );
        if !force {
            return Err(format!("{} Pass force=true to proceed anyway.", msg));
        }
        warning = Some(msg);
    }

    run_search(window, &control, orderings as f64, warning, false, move |cancel, report| {
        search::search_reorder(&indices, search::DEFAULT_MATCH_CAP, cancel, report)
    })
    .await
}

/// Asks the running search to stop. It returns its partial results.
#[tauri::command]
fn cancel_search(control: tauri::State<'_, SearchControl>) {
    control.cancel.store(true, Ordering::SeqCst);
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
        .manage(SearchControl::default())
        .invoke_handler(tauri::generate_handler![
            fix_typos,
            search_missing,
            search_reorder,
            cancel_search,
            check_address,
            validate_mnemonic,
            validate_license,
            derive_seed_hex
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
