"""
bip39.py — Minimal, dependency-free BIP39 implementation.

This is the trust-critical core of the whole project: checksum validation,
mnemonic <-> entropy conversion, and seed derivation. It intentionally has
ZERO network calls and ZERO external dependencies, so it can be read start
to finish by a skeptical reviewer in a few minutes.

Reference: https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki
"""

import hashlib
import hmac
import unicodedata
from pathlib import Path

WORDLIST_PATH = Path(__file__).parent.parent / "wordlists" / "english.txt"

# Valid (word_count, entropy_bits, checksum_bits) combinations per the spec.
VALID_LENGTHS = {
    12: (128, 4),
    15: (160, 5),
    18: (192, 6),
    21: (224, 7),
    24: (256, 8),
}


def load_wordlist(path: Path = WORDLIST_PATH) -> list[str]:
    """Load the BIP39 wordlist from disk. Must contain exactly 2048 words."""
    words = path.read_text(encoding="utf-8").splitlines()
    words = [w.strip() for w in words if w.strip()]
    if len(words) != 2048:
        raise ValueError(
            f"Wordlist at {path} has {len(words)} words, expected 2048. "
            "Do not proceed — this indicates a corrupted or wrong wordlist file."
        )
    return words


WORDLIST = load_wordlist()
WORD_TO_INDEX = {w: i for i, w in enumerate(WORDLIST)}


def entropy_to_checksum_bits(entropy: bytes) -> str:
    """Compute the checksum bits (first N bits of SHA-256(entropy)) for a given entropy."""
    checksum_len = len(entropy) * 8 // 32
    digest = hashlib.sha256(entropy).digest()
    bits = "".join(f"{byte:08b}" for byte in digest)
    return bits[:checksum_len]


def entropy_to_mnemonic(entropy: bytes) -> list[str]:
    """Convert raw entropy bytes into a BIP39 mnemonic word list."""
    entropy_bits = len(entropy) * 8
    if entropy_bits not in {v[0] for v in VALID_LENGTHS.values()}:
        raise ValueError(f"Invalid entropy length: {entropy_bits} bits")

    entropy_bin = "".join(f"{byte:08b}" for byte in entropy)
    checksum_bin = entropy_to_checksum_bits(entropy)
    full_bits = entropy_bin + checksum_bin

    words = []
    for i in range(0, len(full_bits), 11):
        chunk = full_bits[i : i + 11]
        index = int(chunk, 2)
        words.append(WORDLIST[index])
    return words


def normalize_word(word: str) -> str:
    """
    Trim surrounding whitespace and lowercase a word before comparing it
    against the wordlist. The official wordlist is already all-lowercase
    ASCII, so this makes lookup robust to accidental capitalization or
    stray whitespace from copy-paste without changing the underlying BIP39
    semantics at all.
    """
    return word.strip().lower()


def mnemonic_to_entropy_and_checksum(words: list[str]) -> tuple[bytes, str, str]:
    """
    Convert a mnemonic word list back into (entropy_bytes, actual_checksum_bits,
    expected_checksum_bits). Raises ValueError if any word isn't in the wordlist
    or the word count is invalid.

    Word matching is case/whitespace-normalized (see normalize_word) before
    comparing against the wordlist, so " Abandon" and "abandon" resolve
    identically.

    NOTE on memory safety: unlike the Rust implementation (which wraps this
    function's entropy output in Zeroizing so it's cleared from memory when
    dropped), Python strings and bytes objects here are NOT zeroized on
    disposal. CPython's memory model makes reliably zeroing immutable
    string/bytes data impractical without dropping to a C extension, which
    this project intentionally has not done, to keep the reference
    implementation dependency-free and auditable in a few minutes. This is a
    known, documented limitation — see docs/SECURITY.md. The Rust/Tauri GUI
    is the safer choice for real, sensitive use once code review is further
    along.
    """
    n = len(words)
    if n not in VALID_LENGTHS:
        raise ValueError(f"Invalid word count: {n}. Must be one of {sorted(VALID_LENGTHS)}.")

    entropy_bits, checksum_bits = VALID_LENGTHS[n]

    try:
        indices = [WORD_TO_INDEX[normalize_word(w)] for w in words]
    except KeyError as e:
        raise ValueError(f"Word not in BIP39 wordlist: {e.args[0]!r}") from e

    full_bits = "".join(f"{i:011b}" for i in indices)
    entropy_bin = full_bits[:entropy_bits]
    actual_checksum = full_bits[entropy_bits:]

    entropy_bytes = int(entropy_bin, 2).to_bytes(entropy_bits // 8, "big")
    expected_checksum = entropy_to_checksum_bits(entropy_bytes)

    return entropy_bytes, actual_checksum, expected_checksum


def is_valid_mnemonic(words: list[str]) -> bool:
    """The single most important function in this codebase: does this phrase pass BIP39 checksum?"""
    try:
        _, actual, expected = mnemonic_to_entropy_and_checksum(words)
        return actual == expected
    except ValueError:
        return False


def mnemonic_to_seed(words: list[str], passphrase: str = "") -> bytes:
    """
    Derive the 64-byte BIP39 seed from a mnemonic + optional passphrase.
    This does NOT validate the checksum first — callers should call
    is_valid_mnemonic() beforehand if checksum validity matters for their use case.
    """
    mnemonic_str = unicodedata.normalize("NFKD", " ".join(words))
    salt_str = unicodedata.normalize("NFKD", "mnemonic" + passphrase)
    return hashlib.pbkdf2_hmac(
        "sha512",
        mnemonic_str.encode("utf-8"),
        salt_str.encode("utf-8"),
        iterations=2048,
        dklen=64,
    )


def closest_words(word: str, max_results: int = 3) -> list[str]:
    """Find the closest wordlist matches to a possibly-misspelled word, by edit distance."""
    def levenshtein(a: str, b: str) -> int:
        if len(a) < len(b):
            a, b = b, a
        prev = list(range(len(b) + 1))
        for i, ca in enumerate(a, 1):
            curr = [i] + [0] * len(b)
            for j, cb in enumerate(b, 1):
                curr[j] = min(
                    prev[j] + 1,
                    curr[j - 1] + 1,
                    prev[j - 1] + (ca != cb),
                )
            prev = curr
        return prev[-1]

    scored = sorted(WORDLIST, key=lambda w: levenshtein(normalize_word(word), w))
    return scored[:max_results]
