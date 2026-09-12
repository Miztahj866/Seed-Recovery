"""
recover.py — Command-line seed phrase recovery tool.

Handles: typo correction, missing-word brute force, word-order search.
Everything here runs 100% locally — no network calls anywhere in this
file or in bip39.py. You can (and should) verify that yourself.

Usage examples:
    # Typo correction
    python3 recover.py fix "abandan abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

    # One missing word (use ? for the unknown slot)
    python3 recover.py missing "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon ?"

    # Word order unknown (given the correct set of 12/24 words)
    python3 recover.py reorder "about abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon"
"""

import itertools
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from bip39 import WORDLIST, closest_words, is_valid_mnemonic, normalize_word  # noqa: E402

MAX_BLANKS_WARNING = 3
MAX_REORDER_WARNING = 6  # words with genuinely unknown position


def cmd_fix(words: list[str]) -> None:
    """Suggest corrections for words not in the wordlist, then check checksum."""
    fixed = []
    changed = False
    for w in words:
        normalized = normalize_word(w)
        if normalized in WORDLIST:
            fixed.append(normalized)
        else:
            suggestions = closest_words(w, max_results=3)
            print(f"  '{w}' not in wordlist. Closest matches: {', '.join(suggestions)}")
            fixed.append(suggestions[0])
            changed = True

    if not changed:
        print("All words are already valid wordlist entries.")
    else:
        print(f"\nSuggested correction: {' '.join(fixed)}")

    if is_valid_mnemonic(fixed):
        print("✓ This phrase PASSES the BIP39 checksum.")
    else:
        print("✗ This phrase does NOT pass the checksum — do not trust it yet.")
        print("  Try 'missing' mode if you're unsure about a specific word,")
        print("  or double check the suggested corrections above.")


def cmd_missing(words: list[str]) -> None:
    """Brute-force blank slots (marked '?') using checksum validation to prune."""
    blank_positions = [i for i, w in enumerate(words) if w == "?"]
    n_blanks = len(blank_positions)

    if n_blanks == 0:
        print("No '?' slots found. Use '?' to mark each unknown word position.")
        return

    if n_blanks > MAX_BLANKS_WARNING:
        print(
            f"WARNING: {n_blanks} missing words means up to {2048**n_blanks:,} "
            "combinations before checksum pruning. This may take a very long time "
            "or be practically infeasible. Proceeding anyway...\n"
        )

    print(f"Searching {n_blanks} missing word(s) across {len(words)} total words...")
    matches = []
    checked = 0

    for combo in itertools.product(WORDLIST, repeat=n_blanks):
        candidate = words.copy()
        for pos, word in zip(blank_positions, combo):
            candidate[pos] = word
        checked += 1
        if is_valid_mnemonic(candidate):
            matches.append(candidate)

    print(f"\nChecked {checked:,} combinations.")
    if matches:
        print(f"Found {len(matches)} candidate(s) that pass the BIP39 checksum:\n")
        for m in matches:
            print(f"  {' '.join(m)}")
        print(
            "\nIMPORTANT: A passing checksum means the phrase is VALID BIP39, "
            "not that it's YOUR phrase. Verify by deriving the address and "
            "comparing it to one you already know belongs to your wallet."
        )
    else:
        print("No valid combinations found. Double-check the known words for typos.")


def cmd_reorder(words: list[str]) -> None:
    """Brute-force word order when the word set is known but the sequence isn't."""
    n = len(words)
    if n > MAX_REORDER_WARNING:
        total_perms = 1
        for i in range(1, n + 1):
            total_perms *= i
        print(
            f"WARNING: {n} words unordered means {total_perms:,} possible orderings. "
            "This is almost certainly impractical to fully search. Consider whether "
            "you actually know the position of most words and only a few are uncertain."
        )
        confirm = input("Continue anyway? (y/N): ").strip().lower()
        if confirm != "y":
            print("Aborted.")
            return

    print(f"Searching all orderings of {n} words...")
    matches = []
    checked = 0

    for perm in itertools.permutations(words):
        checked += 1
        if is_valid_mnemonic(list(perm)):
            matches.append(list(perm))
        if checked % 500_000 == 0:
            print(f"  ...checked {checked:,} orderings so far")

    print(f"\nChecked {checked:,} orderings.")
    if matches:
        print(f"Found {len(matches)} candidate(s) that pass the BIP39 checksum:\n")
        for m in matches:
            print(f"  {' '.join(m)}")
    else:
        print("No valid orderings found among the words provided.")


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        sys.exit(1)

    mode = sys.argv[1]
    words = sys.argv[2].strip().split()

    if mode == "fix":
        cmd_fix(words)
    elif mode == "missing":
        cmd_missing(words)
    elif mode == "reorder":
        cmd_reorder(words)
    else:
        print(f"Unknown mode: {mode}")
        print(__doc__)
        sys.exit(1)


if __name__ == "__main__":
    main()
