"""
test_vectors.py — Validate bip39.py against the official Trezor/BIP39
English reference test vectors.

Source: https://github.com/trezor/python-mnemonic/blob/master/vectors.json
This is THE standard reference set used by virtually every BIP39
implementation (Go, Rust, JS, etc.) to prove correctness. If our
implementation doesn't match these, nothing else in this project can
be trusted.

Run with: python3 tests/test_vectors.py
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from bip39 import (  # noqa: E402
    entropy_to_mnemonic,
    is_valid_mnemonic,
    mnemonic_to_entropy_and_checksum,
    mnemonic_to_seed,
)

# (entropy_hex, mnemonic, seed_hex_with_TREZOR_passphrase) — English vectors
# from trezor/python-mnemonic vectors.json, verbatim.
VECTORS = [
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
        "9e885d952ad362caeb4efe34a8e91bd2",
        "ozone drill grab fiber curtain grace pudding thank cruise elder eight picnic",
        "274ddc525802f7c828d8ef7ddbcdc5304e87ac3535913611fbbfa986d0c9e5476c91689f9c8a54fd55bd38606aa6a8595ad213d4c9c9f9aca3fb217069a41028",
    ),
    (
        "f30f8c1da665478f49b001d94c5fc452",
        "vessel ladder alter error federal sibling chat ability sun glass valve picture",
        "2aaa9242daafcee6aa9d7269f17d4efe271e1b9a529178d7dc139cd18747090bf9d60295d0ce74309a78852a9caadf0af48aae1c6253839624076224374bc63f",
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
]

PASSPHRASE = "TREZOR"


def run():
    failures = 0
    total = 0

    for entropy_hex, expected_mnemonic, expected_seed_hex in VECTORS:
        entropy = bytes.fromhex(entropy_hex)
        expected_words = expected_mnemonic.split()

        # Test 1: entropy -> mnemonic
        total += 1
        actual_words = entropy_to_mnemonic(entropy)
        if actual_words != expected_words:
            print(f"FAIL [entropy->mnemonic] {entropy_hex[:16]}...")
            print(f"  expected: {' '.join(expected_words)}")
            print(f"  actual:   {' '.join(actual_words)}")
            failures += 1
        else:
            print(f"PASS [entropy->mnemonic] {entropy_hex[:16]}... ({len(expected_words)} words)")

        # Test 2: mnemonic -> entropy + checksum validity
        total += 1
        recovered_entropy, actual_checksum, expected_checksum = mnemonic_to_entropy_and_checksum(
            expected_words
        )
        if recovered_entropy != entropy or actual_checksum != expected_checksum:
            print(f"FAIL [mnemonic->entropy] {entropy_hex[:16]}...")
            failures += 1
        else:
            print(f"PASS [mnemonic->entropy] {entropy_hex[:16]}...")

        # Test 3: checksum validation accepts the correct phrase
        total += 1
        if not is_valid_mnemonic(expected_words):
            print(f"FAIL [is_valid_mnemonic should be True] {entropy_hex[:16]}...")
            failures += 1
        else:
            print(f"PASS [is_valid_mnemonic == True] {entropy_hex[:16]}...")

        # Test 4: seed derivation with the standard "TREZOR" test passphrase
        total += 1
        seed = mnemonic_to_seed(expected_words, PASSPHRASE)
        if seed.hex() != expected_seed_hex:
            print(f"FAIL [mnemonic->seed] {entropy_hex[:16]}...")
            print(f"  expected: {expected_seed_hex}")
            print(f"  actual:   {seed.hex()}")
            failures += 1
        else:
            print(f"PASS [mnemonic->seed] {entropy_hex[:16]}...")

    # Negative test: corrupting one word should break the checksum
    total += 1
    corrupted = VECTORS[0][1].split()
    corrupted[-1] = "zoo" if corrupted[-1] != "zoo" else "about"
    if is_valid_mnemonic(corrupted):
        print("FAIL [corrupted phrase should be INVALID but passed checksum]")
        failures += 1
    else:
        print("PASS [corrupted phrase correctly rejected by checksum]")

    # Case/whitespace normalization test: messy input should resolve identically
    # to the clean version -- this is the fix for words not being recognized
    # when pasted with inconsistent casing or stray spaces.
    total += 1
    clean_words = VECTORS[0][1].split()
    messy_words = [f" {clean_words[0].upper()}"] + clean_words[1:-1] + [f"{clean_words[-1].title()} "]
    if not is_valid_mnemonic(messy_words):
        print("FAIL [messy-cased phrase should still validate]")
        failures += 1
    else:
        messy_entropy, _, _ = mnemonic_to_entropy_and_checksum(messy_words)
        clean_entropy, _, _ = mnemonic_to_entropy_and_checksum(clean_words)
        if messy_entropy != clean_entropy:
            print("FAIL [messy and clean casing produced different entropy]")
            failures += 1
        else:
            print("PASS [case/whitespace-insensitive wordlist lookup]")

    print()
    print(f"{total - failures}/{total} checks passed")
    if failures:
        print(f"{failures} FAILURE(S) — do not trust this implementation until fixed.")
        sys.exit(1)
    else:
        print("All official BIP39 test vectors passed.")


if __name__ == "__main__":
    run()
