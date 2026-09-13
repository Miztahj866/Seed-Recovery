//! gen_license.rs — Seller-side tool for the license key system.
//!
//! This binary is NOT part of the shipped app. It's a tool you (the seller)
//! run on your own machine to:
//!   1. Generate a signing keypair (once, ever)
//!   2. Issue individual license keys after each sale
//!
//! The private key file this produces must NEVER be committed to git,
//! shared, or shipped with the app. Only the PUBLIC key gets pasted into
//! `src/license.rs`'s `PUBLIC_KEY_BYTES` constant.
//!
//! Usage:
//!   cargo run --bin gen_license -- genkey
//!       Generates a new keypair. Prints the public key bytes to paste into
//!       license.rs, and saves the private key to ./signing_key.bin
//!       (add this filename to .gitignore immediately if you haven't).
//!
//!   cargo run --bin gen_license -- issue <license_id>
//!       Reads ./signing_key.bin and produces a license key string for the
//!       given license_id (use an order ID, email hash, or any identifier
//!       you want to associate with this sale). Prints the key to give to
//!       the customer.

use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use std::env;
use std::fs;

const KEY_FILE: &str = "signing_key.bin";

fn cmd_genkey() {
    if fs::metadata(KEY_FILE).is_ok() {
        eprintln!(
            "Error: {KEY_FILE} already exists. Refusing to overwrite an existing signing key \
             (that would invalidate every license you've already issued). Delete it manually \
             first if you're sure you want a fresh keypair."
        );
        std::process::exit(1);
    }

    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();

    fs::write(KEY_FILE, signing_key.to_bytes())
        .unwrap_or_else(|e| panic!("Failed to write {KEY_FILE}: {e}"));

    println!("Keypair generated.");
    println!();
    println!("Private key saved to: {KEY_FILE}");
    println!("  -> NEVER commit this file to git. NEVER share it. Back it up somewhere safe");
    println!("     and private -- if you lose it, you can't issue new licenses under the same");
    println!("     public key, and every license you've already issued still works fine (it");
    println!("     doesn't need the private key to keep validating).");
    println!();
    println!("Public key (paste this into src/license.rs's PUBLIC_KEY_BYTES):");
    println!();
    print!("const PUBLIC_KEY_BYTES: [u8; 32] = [");
    for (i, b) in verifying_key.to_bytes().iter().enumerate() {
        if i > 0 {
            print!(", ");
        }
        print!("{b}");
    }
    println!("];");
}

fn cmd_issue(license_id: &str) {
    let key_bytes = fs::read(KEY_FILE).unwrap_or_else(|_| {
        eprintln!(
            "Error: {KEY_FILE} not found. Run 'cargo run --bin gen_license -- genkey' first."
        );
        std::process::exit(1);
    });
    let key_array: [u8; 32] = key_bytes
        .try_into()
        .unwrap_or_else(|_| panic!("{KEY_FILE} is corrupted (wrong length)."));
    let signing_key = SigningKey::from_bytes(&key_array);

    let payload = license_id.as_bytes();
    let signature = signing_key.sign(payload);

    let license_key = format!(
        "{}.{}",
        STANDARD.encode(payload),
        STANDARD.encode(signature.to_bytes())
    );

    println!("License key for '{license_id}':");
    println!();
    println!("{license_key}");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("genkey") => cmd_genkey(),
        Some("issue") => {
            let license_id = args.get(2).unwrap_or_else(|| {
                eprintln!("Usage: gen_license issue <license_id>");
                std::process::exit(1);
            });
            cmd_issue(license_id);
        }
        _ => {
            eprintln!("Usage:");
            eprintln!("  gen_license genkey              Generate a new signing keypair");
            eprintln!("  gen_license issue <license_id>  Issue a license key");
            std::process::exit(1);
        }
    }
}
