//! search.rs — the brute-force search engine behind "Missing Word(s)" and
//! "Word Order".
//!
//! Deliberately free of any Tauri/GUI code so it can be unit-tested on its
//! own. The app layer (main.rs) runs these functions on a background thread,
//! forwards progress to the window, and flips the `cancel` flag when the
//! user presses Cancel.
//!
//! Three properties matter here, and the tests at the bottom check them:
//!
//! 1. **Same answers as `bip39::is_valid_mnemonic`.** The search uses a
//!    faster, index-based checksum (`checksum_ok`), which is cross-checked
//!    against the reference implementation in `bip39.rs`.
//! 2. **Bounded memory.** Only the first `cap` matches are kept. The total
//!    number of matches is still counted, so the user knows how many exist.
//! 3. **Stoppable.** Every worker checks the `cancel` flag regularly and
//!    returns promptly once it is set.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use sha2::{Digest, Sha256};

use crate::bip39;

/// How many matching phrases are kept and sent back to the window.
/// Everything beyond this is counted but not stored.
pub const DEFAULT_MATCH_CAP: usize = 1000;

/// How many candidates a worker checks between looking at the cancel flag
/// and reporting progress. Small enough that Cancel feels instant, large
/// enough that the shared counters aren't a bottleneck.
const BATCH: u64 = 4096;

const WORDS: u16 = 2048;

/// The result of a finished (or cancelled) search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Candidates actually checked. Less than `total` if cancelled.
    pub checked: u64,
    /// Size of the full search space (2048^blanks, or n! for word order).
    pub total: u128,
    /// How many candidates passed the checksum, including ones not stored.
    pub match_count: u64,
    /// Up to `cap` matching phrases, as wordlist indices, sorted.
    pub matches: Vec<Vec<u16>>,
    pub cancelled: bool,
}

impl Outcome {
    pub fn truncated(&self) -> bool {
        self.match_count > self.matches.len() as u64
    }
}

fn word_index_map() -> &'static HashMap<&'static str, u16> {
    static MAP: OnceLock<HashMap<&'static str, u16>> = OnceLock::new();
    MAP.get_or_init(|| {
        bip39::wordlist()
            .into_iter()
            .enumerate()
            .map(|(i, w)| (w, i as u16))
            .collect()
    })
}

/// The wordlist itself, built once.
fn words() -> &'static [&'static str] {
    static LIST: OnceLock<Vec<&'static str>> = OnceLock::new();
    LIST.get_or_init(bip39::wordlist)
}

/// Turn wordlist indices back into words.
pub fn indices_to_words(idx: &[u16]) -> Vec<String> {
    let wl = words();
    idx.iter().map(|&i| wl[i as usize].to_string()).collect()
}

/// Returns (entropy bits, checksum bits) for a valid BIP39 length.
fn lengths(word_count: usize) -> Option<(usize, usize)> {
    match word_count {
        12 => Some((128, 4)),
        15 => Some((160, 5)),
        18 => Some((192, 6)),
        21 => Some((224, 7)),
        24 => Some((256, 8)),
        _ => None,
    }
}

/// Parse the user's words. `?` marks an unknown position (`None`).
/// Fails with a readable message for a bad length or a word that isn't on
/// the BIP39 list, so a long search never starts on input that can't match.
pub fn parse_template(input: &[String]) -> Result<Vec<Option<u16>>, String> {
    if lengths(input.len()).is_none() {
        return Err(format!(
            "A BIP39 phrase has 12, 15, 18, 21 or 24 words. You entered {}. \
             Count the '?' slots too.",
            input.len()
        ));
    }
    let map = word_index_map();
    input
        .iter()
        .enumerate()
        .map(|(pos, raw)| {
            let w = raw.trim().to_lowercase();
            if w == "?" {
                return Ok(None);
            }
            map.get(w.as_str()).copied().map(Some).ok_or_else(|| {
                format!(
                    "Word {} ('{}') isn't on the BIP39 word list. \
                     Fix it on the Fix Typos tab first, or mark it with '?'.",
                    pos + 1,
                    raw.trim()
                )
            })
        })
        .collect()
}

/// Fast BIP39 checksum test on wordlist indices.
///
/// Packs the 11-bit indices into bytes, hashes the entropy part with
/// SHA-256, and compares the leading checksum bits. Same result as
/// `bip39::is_valid_mnemonic`, without the string handling, so it is fast
/// enough to run millions of times.
pub fn checksum_ok(idx: &[u16]) -> bool {
    let (ent_bits, cs_bits) = match lengths(idx.len()) {
        Some(l) => l,
        None => return false,
    };
    // 24 words * 11 bits = 264 bits = 33 bytes, the largest case.
    let mut bytes = [0u8; 33];
    let mut acc: u32 = 0;
    let mut nbits: u32 = 0;
    let mut pos = 0;
    for &v in idx {
        acc = (acc << 11) | (v as u32 & 0x7FF);
        nbits += 11;
        while nbits >= 8 {
            nbits -= 8;
            bytes[pos] = (acc >> nbits) as u8;
            pos += 1;
        }
        acc &= (1 << nbits) - 1;
    }
    if nbits > 0 {
        bytes[pos] = (acc << (8 - nbits)) as u8;
    }
    let ent_bytes = ent_bits / 8;
    let hash = Sha256::digest(&bytes[..ent_bytes]);
    let shift = 8 - cs_bits as u32;
    (bytes[ent_bytes] >> shift) == (hash[0] >> shift)
}

/// Shared bookkeeping for one search, used by every worker thread.
struct Tally<'a, F: Fn(u64) + Sync> {
    cap: usize,
    cancel: &'a AtomicBool,
    progress: F,
    checked: AtomicU64,
    match_count: AtomicU64,
    matches: Mutex<Vec<Vec<u16>>>,
}

impl<'a, F: Fn(u64) + Sync> Tally<'a, F> {
    fn new(cap: usize, cancel: &'a AtomicBool, progress: F) -> Self {
        Tally {
            cap,
            cancel,
            progress,
            checked: AtomicU64::new(0),
            match_count: AtomicU64::new(0),
            matches: Mutex::new(Vec::new()),
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn record(&self, candidate: &[u16]) {
        self.match_count.fetch_add(1, Ordering::Relaxed);
        let mut m = self.matches.lock().unwrap();
        if m.len() < self.cap {
            m.push(candidate.to_vec());
        }
    }

    /// Add a finished batch to the shared count and report progress.
    fn add_checked(&self, n: u64) {
        if n > 0 {
            let total = self.checked.fetch_add(n, Ordering::Relaxed) + n;
            (self.progress)(total);
        }
    }

    fn finish(self, total: u128) -> Outcome {
        let mut matches = self.matches.into_inner().unwrap();
        matches.sort();
        matches.dedup();
        Outcome {
            checked: self.checked.into_inner(),
            total,
            match_count: self.match_count.into_inner(),
            matches,
            cancelled: self.cancel.load(Ordering::Relaxed),
        }
    }
}

/// Run `job(i)` for every `i` in `0..count`, spread over all CPU cores.
///
/// Uses only the standard library (scoped threads), so the search adds no
/// new dependencies. Threads pull the next chunk from a shared counter, so
/// a slow chunk never leaves other cores idle.
fn par_for_each(count: usize, job: impl Fn(usize) + Sync) {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(count.max(1));
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= count {
                    break;
                }
                job(i);
            });
        }
    });
}

/// Try every word in every `?` slot and keep the phrases that pass the
/// checksum.
///
/// The space is split across CPU cores by the first unknown slot's word
/// (2048 independent chunks), so every core stays busy.
/// `progress` is called often with the running count of checked candidates;
/// the caller decides how often to actually redraw.
pub fn search_missing<F>(template: &[Option<u16>], cap: usize, cancel: &AtomicBool, progress: F) -> Outcome
where
    F: Fn(u64) + Sync,
{
    let blanks: Vec<usize> = template
        .iter()
        .enumerate()
        .filter(|(_, w)| w.is_none())
        .map(|(i, _)| i)
        .collect();
    let base: Vec<u16> = template.iter().map(|w| w.unwrap_or(0)).collect();
    let total = (WORDS as u128).pow(blanks.len() as u32);
    let tally = Tally::new(cap, cancel, progress);

    if blanks.is_empty() {
        tally.add_checked(1);
        if checksum_ok(&base) {
            tally.record(&base);
        }
        return tally.finish(total);
    }

    let first = blanks[0];
    let rest = &blanks[1..];

    par_for_each(WORDS as usize, |w0| {
        let w0 = w0 as u16;
        if tally.cancelled() {
            return;
        }
        let mut cand = base.clone();
        cand[first] = w0;
        for &p in rest {
            cand[p] = 0;
        }
        let mut local: u64 = 0;
        loop {
            local += 1;
            if checksum_ok(&cand) {
                tally.record(&cand);
            }
            if local == BATCH {
                tally.add_checked(local);
                local = 0;
                if tally.cancelled() {
                    return;
                }
            }
            // Odometer over the remaining slots, rightmost turning fastest.
            let mut i = rest.len();
            loop {
                if i == 0 {
                    tally.add_checked(local);
                    return;
                }
                i -= 1;
                let p = rest[i];
                cand[p] += 1;
                if cand[p] < WORDS {
                    break;
                }
                cand[p] = 0;
            }
        }
    });

    tally.finish(total)
}

fn factorial(n: usize) -> u128 {
    (1..=n as u128).product()
}

/// Visit every ordering of `cand[start..]` exactly once (Heap's algorithm),
/// leaving `cand[..start]` fixed. `visit` returns false to stop early;
/// the function returns false if it was stopped.
fn heap_walk(cand: &mut [u16], start: usize, visit: &mut impl FnMut(&[u16]) -> bool) -> bool {
    let m = cand.len().saturating_sub(start);
    if !visit(cand) {
        return false;
    }
    let mut c = vec![0usize; m];
    let mut i = 0;
    while i < m {
        if c[i] < i {
            if i % 2 == 0 {
                cand.swap(start, start + i);
            } else {
                cand.swap(start + c[i], start + i);
            }
            if !visit(cand) {
                return false;
            }
            c[i] += 1;
            i = 0;
        } else {
            c[i] = 0;
            i += 1;
        }
    }
    true
}

/// Try every ordering of the given words and keep the ones that pass the
/// checksum.
///
/// Split across cores by which word goes first; each core then walks the
/// orderings of the remaining words with Heap's algorithm. Repeated words
/// produce repeated orderings, so `match_count` counts orderings checked
/// while `matches` holds each distinct phrase once.
pub fn search_reorder<F>(idx: &[u16], cap: usize, cancel: &AtomicBool, progress: F) -> Outcome
where
    F: Fn(u64) + Sync,
{
    let n = idx.len();
    let total = factorial(n);
    let tally = Tally::new(cap, cancel, progress);
    if n == 0 {
        return tally.finish(total);
    }

    par_for_each(n, |lead| {
        if tally.cancelled() {
            return;
        }
        let mut cand: Vec<u16> = Vec::with_capacity(n);
        cand.push(idx[lead]);
        cand.extend(idx.iter().enumerate().filter(|(i, _)| *i != lead).map(|(_, &v)| v));

        let mut local: u64 = 0;
        let finished = heap_walk(&mut cand, 1, &mut |c: &[u16]| {
            local += 1;
            if checksum_ok(c) {
                tally.record(c);
            }
            if local == BATCH {
                tally.add_checked(local);
                local = 0;
                return !tally.cancelled();
            }
            true
        });
        if finished {
            tally.add_checked(local);
        }
    });

    tally.finish(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(phrase: &str) -> Vec<String> {
        phrase.split_whitespace().map(String::from).collect()
    }

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    const ABOUT: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    // Official Trezor/BIP39 vector (24 words).
    const ZOO24: &str = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote";
    // Official Trezor/BIP39 vectors (12, 18 words).
    const LEGAL12: &str = "legal winner thank year wave sausage worth useful legal winner thank yellow";
    const LETTER18: &str = "letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter always";

    fn idx(phrase: &str) -> Vec<u16> {
        parse_template(&s(phrase)).unwrap().into_iter().map(Option::unwrap).collect()
    }

    #[test]
    fn fast_checksum_agrees_with_reference_on_vectors() {
        for p in [ABOUT, ZOO24, LEGAL12, LETTER18] {
            assert!(checksum_ok(&idx(p)), "{p}");
            assert!(bip39::is_valid_mnemonic(&s(p)));
        }
    }

    #[test]
    fn fast_checksum_agrees_with_reference_on_every_last_word() {
        // Every possible last word for two phrases: 2 x 2048 cases, both
        // passing and failing, must match the reference exactly.
        for p in [ABOUT, ZOO24] {
            let mut words = s(p);
            let n = words.len();
            let wl = bip39::wordlist();
            for w in wl {
                words[n - 1] = w.to_string();
                let fast = checksum_ok(&parse_template(&words).unwrap().into_iter().map(Option::unwrap).collect::<Vec<_>>());
                assert_eq!(fast, bip39::is_valid_mnemonic(&words), "last word {w}");
            }
        }
    }

    #[test]
    fn parse_rejects_bad_length_and_unknown_words() {
        assert!(parse_template(&s("abandon ? about")).is_err());
        let mut w = s(ABOUT);
        w[3] = "abandn".into();
        let err = parse_template(&w).unwrap_err();
        assert!(err.contains("Word 4"), "{err}");
        w[3] = " Abandon ".into();
        assert!(parse_template(&w).is_ok());
    }

    #[test]
    fn one_missing_word_finds_original_among_128() {
        let mut w = s(ABOUT);
        w[5] = "?".into();
        let out = search_missing(&parse_template(&w).unwrap(), DEFAULT_MATCH_CAP, &no_cancel(), |_| {});
        assert_eq!(out.checked, 2048);
        assert_eq!(out.total, 2048);
        assert!(!out.cancelled);
        // 12 words have a 4-bit checksum: about 1 in 16 candidates pass.
        assert!((100..=160).contains(&out.match_count), "{}", out.match_count);
        assert!(out.matches.contains(&idx(ABOUT)));
        assert_eq!(out.match_count as usize, out.matches.len());
    }

    #[test]
    fn missing_word_results_match_reference_exactly() {
        let mut w = s(LEGAL12);
        w[11] = "?".into();
        let out = search_missing(&parse_template(&w).unwrap(), DEFAULT_MATCH_CAP, &no_cancel(), |_| {});
        let expected: Vec<Vec<u16>> = bip39::wordlist()
            .iter()
            .filter_map(|cand| {
                let mut t = w.clone();
                t[11] = cand.to_string();
                bip39::is_valid_mnemonic(&t).then(|| idx(&t.join(" ")))
            })
            .collect();
        assert_eq!(out.matches, expected);
    }

    #[test]
    fn two_missing_words_on_24_words() {
        let mut w = s(ZOO24);
        w[2] = "?".into();
        w[20] = "?".into();
        let out = search_missing(&parse_template(&w).unwrap(), DEFAULT_MATCH_CAP, &no_cancel(), |_| {});
        assert_eq!(out.checked, 2048 * 2048);
        // 8-bit checksum: about 4,194,304 / 256 = 16,384 pass.
        assert!((15_000..=18_000).contains(&out.match_count), "{}", out.match_count);
        assert!(out.truncated());
        assert_eq!(out.matches.len(), DEFAULT_MATCH_CAP);
    }

    #[test]
    fn cap_limits_stored_matches_but_not_the_count() {
        let mut w = s(ABOUT);
        w[0] = "?".into();
        let out = search_missing(&parse_template(&w).unwrap(), 5, &no_cancel(), |_| {});
        assert_eq!(out.matches.len(), 5);
        assert!(out.match_count > 5);
        assert!(out.truncated());
    }

    #[test]
    fn cancel_stops_a_large_search_early() {
        let mut w = s(ZOO24);
        for i in [1, 4, 9] {
            w[i] = "?".into();
        }
        // 2048^3 = 8.6 billion candidates; cancel once the first progress
        // report arrives and make sure we stop far short of that.
        let cancel = AtomicBool::new(false);
        let out = search_missing(&parse_template(&w).unwrap(), 10, &cancel, |_| {
            cancel.store(true, Ordering::Relaxed);
        });
        assert!(out.cancelled);
        assert_eq!(out.total, 2048u128.pow(3));
        assert!(out.checked < 50_000_000, "{}", out.checked);
    }

    #[test]
    fn progress_reaches_the_total() {
        let mut w = s(LEGAL12);
        w[0] = "?".into();
        let seen = AtomicU64::new(0);
        let out = search_missing(&parse_template(&w).unwrap(), 10, &no_cancel(), |c| {
            seen.fetch_max(c, Ordering::Relaxed);
        });
        assert_eq!(seen.load(Ordering::Relaxed), out.checked);
        assert_eq!(out.checked, 2048);
    }

    #[test]
    fn heap_walk_visits_every_ordering_once() {
        use std::collections::HashSet;
        let mut cand: Vec<u16> = (0..7).collect();
        let mut seen = HashSet::new();
        let done = heap_walk(&mut cand, 1, &mut |c: &[u16]| {
            assert_eq!(c[0], 0, "fixed prefix must not move");
            assert!(seen.insert(c.to_vec()), "ordering visited twice: {c:?}");
            true
        });
        assert!(done);
        assert_eq!(seen.len(), 720); // 6! orderings of the last 6
    }

    #[test]
    fn heap_walk_stops_when_asked() {
        let mut cand: Vec<u16> = (0..8).collect();
        let mut n = 0;
        let done = heap_walk(&mut cand, 0, &mut |_| {
            n += 1;
            n < 10
        });
        assert!(!done);
        assert_eq!(n, 10);
    }

    #[test]
    fn reorder_checks_every_ordering() {
        let words: Vec<u16> = (100..106).collect();
        let out = search_reorder(&words, 10, &no_cancel(), |_| {});
        assert_eq!(out.checked, 720); // 6!
        assert_eq!(out.total, 720);
        assert_eq!(out.match_count, 0); // 6 words is never a valid length
        assert!(!out.cancelled);
    }

    #[test]
    fn reorder_matches_are_real_and_cancel_works() {
        // 12! = 479 million orderings is too slow for a unit test, so run
        // for a while, cancel, and check what was found.
        let mut words = idx(LEGAL12);
        words.swap(0, 11);
        let cancel = AtomicBool::new(false);
        let out = search_reorder(&words, 50, &cancel, |c| {
            if c > 1_000_000 {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert!(out.cancelled);
        assert_eq!(out.total, factorial(12));
        assert!(out.checked < 50_000_000, "{}", out.checked);
        assert!(out.match_count > 0);
        for m in &out.matches {
            assert!(bip39::is_valid_mnemonic(&indices_to_words(m)));
        }
    }
}
