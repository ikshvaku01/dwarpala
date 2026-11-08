//! Dwarpala license signing tool.
//!
//! USAGE
//!   sign_license keygen [--kid k1]
//!       → generate keypair; prints ObfKey{} block for license.rs + private key
//!
//!   sign_license sign --key <b64_private> --tier pro|team|enterprise \
//!       --sub user@example.com [--org "Acme"] [--exp 2026-12-31] \
//!       [--note "…"] [--out license.license] [--kid k1]
//!       → writes a .license JSON file (or prints to stdout)
//!
//!   sign_license inspect <token_or_file>
//!       → decode and display claims (no sig check — use the enterprise binary)
//!
//! SECURITY: The private key never leaves this tool. Never commit it.

mod keystore;
use keystore::{from_hex32, to_hex, Entry};
use std::path::Path;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine as _;
use clap::{Parser, Subcommand};
use ed25519_dalek::{SigningKey, Signer, VerifyingKey, Signature};
use rand::{rngs::OsRng, RngCore};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::Deserialize;

#[derive(Deserialize)]
struct TokenHeader {
    alg: String,
    kid: String,
}

#[derive(Deserialize)]
struct LicenseClaims {
    tier: String,
    sub:  String,
    #[serde(default)]
    org:  String,
    exp:  Option<i64>,
    iat:  i64,
    jti:  String,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize)]
struct LicenseFile {
    #[allow(dead_code)]
    version: u32,
    product: String,
    token:   String,
    #[allow(dead_code)]
    #[serde(default)]
    display: Value,
}

fn die(msg: &str) -> ! { eprintln!("❌ {msg}"); std::process::exit(1) }

fn main() {
    let cli = Cli::parse();
    let kp = keystore::resolve_path(cli.keys.as_deref());
    match cli.cmd {
        Cmd::Keygen { kid, yes } => keygen(&kp, kid.as_deref(), yes),
        Cmd::Sign { key, kid, tier, sub, org, exp, note, out } => sign(
            &kp, key.as_deref(), kid.as_deref(), &tier, &sub, &org,
            exp.as_deref(), note.as_deref(), out.as_deref()),
        Cmd::Verify { input, kid } => verify(&kp, &input, kid.as_deref()),
        Cmd::Inspect { input } => inspect(&input),
    }
}

#[derive(Parser)]
#[command(name = "sign_license", about = "Dwarpala license signing tool")]
struct Cli {
    /// Key store (default: $DWARPALA_KEYS_FILE, else <config dir>/dwarpala/license-keys.json)
    #[arg(long, global = true)] keys: Option<String>,
    #[command(subcommand)] cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a new Ed25519 keypair
    Keygen {
        /// Omit to auto-pick the next unused k<N>
        #[arg(long)] kid: Option<String>,
        /// Save without asking
        #[arg(long, short = 'y')] yes: bool,
    },
    /// Sign and issue a new license
    Sign {
        /// Omit to use the private key stored for --kid
        #[arg(long)] key: Option<String>,
        /// Omit to use the newest enabled key
        #[arg(long)] kid: Option<String>,
        /// pro | team | enterprise
        #[arg(long)] tier: String,
        /// Subject (email or identifier)
        #[arg(long)] sub:  String,
        #[arg(long, default_value = "")] org: String,
        /// Expiry YYYY-MM-DD; omit for perpetual
        #[arg(long)] exp:  Option<String>,
        #[arg(long)] note: Option<String>,
        /// Write to this .license file; print JSON to stdout if omitted
        #[arg(long)] out:  Option<String>,
    },
    Verify {
        /// Token string or path to .license file
        input: String,
        #[arg(long)] kid: Option<String>,
    },
    Inspect { input: String },
}

fn keygen(keys_path: &Path, kid: Option<&str>, yes: bool) {
    let mut store = keystore::load(keys_path).unwrap_or_else(|e| die(&e));
    let kid = kid.map(String::from).unwrap_or_else(|| store.next_kid());
    if store.get(&kid).is_some() {
        die(&format!("kid \"{kid}\" already exists in {}", keys_path.display()));
    }

    let sk   = SigningKey::generate(&mut OsRng);
    let pub_ = sk.verifying_key().to_bytes();
    let mut mask = [0u8; 32];
    OsRng.fill_bytes(&mut mask);
    let enc: [u8; 32] = std::array::from_fn(|i| pub_[i] ^ mask[i]);

    let entry = Entry {
        kid: kid.clone(),
        generated: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        disable: false,
        enc: to_hex(&enc),
        mask: to_hex(&mask),
        privatekey: B64.encode(sk.to_bytes()),
    };

    println!("Generated key  kid={kid}  generated={}", entry.generated);
    println!("  enc : {}\n  mask: {}", entry.enc, entry.mask);

    print!("Save {kid} to {} ? [y/N] ", keys_path.display());
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let save = yes || {
        let mut s = String::new();
        std::io::stdin().read_line(&mut s).is_ok() && matches!(s.trim(), "y" | "Y" | "yes")
    };

    if save {
        store.keys.push(entry);
        keystore::save(keys_path, &store).unwrap_or_else(|e| die(&e));
        println!("✓ saved. Rebuild enterprise to embed the public key.");
    } else {
        println!("Not saved. This is the only time the private key is shown:\n{}",
                 serde_json::to_string_pretty(&entry).unwrap());
    }
}

// ── sign ─────────────────────────────────────────────────────────────────────
fn sign(
    keys_path: &Path, key_arg: Option<&str>, kid_arg: Option<&str>,
    tier: &str, sub: &str, org: &str,
    exp: Option<&str>, note: Option<&str>, out: Option<&str>,
) {
    let store = keystore::load(keys_path).unwrap_or_else(|e| die(&e));
    let (kid, key_b64): (String, String) = match key_arg {
        Some(k) => (
            kid_arg.map(String::from)
                .unwrap_or_else(|| die("--kid is required when --key is given")),
            k.to_string(),
        ),
        None => {
            let e = match kid_arg {
                Some(k) => store.get(k).unwrap_or_else(||
                    die(&format!("kid \"{k}\" not found in {}", keys_path.display()))),
                None => store.newest_signing_key().unwrap_or_else(||
                    die(&format!("no usable signing key in {} - run keygen", keys_path.display()))),
            };
            if e.disable { die(&format!("key \"{}\" is disabled", e.kid)); }
            if e.privatekey.is_empty() { die(&format!("no private key stored for \"{}\"", e.kid)); }
            (e.kid.clone(), e.privatekey.clone())
        }
    };

    let key_seed = B64.decode(key_b64.trim()).expect("Invalid base64 private key");
    let key_arr: [u8; 32] = key_seed.as_slice().try_into().expect("Private key must be 32 bytes");
    let sk = SigningKey::from_bytes(&key_arr);

    // Catch a private key that doesn't belong to this kid's stored public key.
    if let Some(e) = store.get(&kid) {
        if let (Some(enc), Some(mask)) = (from_hex32(&e.enc), from_hex32(&e.mask)) {
            let expect: [u8; 32] = std::array::from_fn(|i| enc[i] ^ mask[i]);
            if sk.verifying_key().to_bytes() != expect {
                die(&format!("private key does not match the stored public key for \"{kid}\""));
            }
        }
    }

    // Validate tier
    let tier_lc = tier.to_lowercase();
    if !["pro", "team", "enterprise"].contains(&tier_lc.as_str()) {
        eprintln!("❌ tier must be one of: pro, team, enterprise");
        std::process::exit(1);
    }

    // Parse optional expiry
    let exp_ts: Option<i64> = exp.map(|s| parse_date_to_unix(s));

    // Build unique token id
    let mut jti_buf = [0u8; 12];
    OsRng.fill_bytes(&mut jti_buf);
    let jti = B64.encode(jti_buf);

    let iat = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

    let header = json!({ "alg": "EdDSA", "kid": kid });
    let mut claims = json!({
        "tier": tier_lc,
        "sub":  sub,
        "org":  org,
        "iat":  iat,
        "jti":  jti,
    });
    if let Some(e) = exp_ts { claims["exp"] = json!(e); }
    if let Some(n) = note   { claims["note"] = json!(n); }

    let hdr_b64    = B64.encode(serde_json::to_vec(&header).unwrap());
    let claims_b64 = B64.encode(serde_json::to_vec(&claims).unwrap());
    let message    = format!("DW.{hdr_b64}.{claims_b64}");
    let sig_b64    = B64.encode(sk.sign(message.as_bytes()).to_bytes());
    let token      = format!("DW.{hdr_b64}.{claims_b64}.{sig_b64}");

    let tier_display = match tier_lc.as_str() {
        "pro"        => "Pro",
        "team"       => "Team",
        "enterprise" => "Enterprise",
        other        => other,
    };
    let license_json = json!({
        "version": 1,
        "product": "dwarpala",
        "token":   token,
        "display": {
            "issued_to": sub,
            "org":       org,
            "tier":      tier_display,
            "expires":   exp.unwrap_or("perpetual"),
            "note":      note.unwrap_or("")
        }
    });
    let pretty = serde_json::to_string_pretty(&license_json).unwrap();

    match out {
        Some(path) => {
            std::fs::write(path, &pretty).expect("Failed to write .license file");
            println!("✓ {path}  [{tier_display} · {}]",
                     exp.unwrap_or("perpetual"));
        }
        None => println!("{pretty}"),
    }
}

// ── inspect ──────────────────────────────────────────────────────────────────

fn inspect(input: &str) {
    let token = if Path::new(input).is_file() {
        let raw = std::fs::read_to_string(input).expect("Cannot read file");
        let v: Value = serde_json::from_str(&raw).expect("Invalid JSON");
        v["token"].as_str().expect("No 'token' field").to_string()
    } else {
        input.to_string()
    };

    let parts: Vec<&str> = token.trim().splitn(4, '.').collect();
    if parts.len() != 4 || parts[0] != "DW" {
        eprintln!("Not a valid DW token");
        std::process::exit(1);
    }

    println!("Header:");
    if let Ok(b) = B64.decode(parts[1]) {
        if let Ok(v) = serde_json::from_slice::<Value>(&b) {
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        }
    }
    println!("\nClaims:");
    if let Ok(b) = B64.decode(parts[2]) {
        if let Ok(v) = serde_json::from_slice::<Value>(&b) {
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
        }
    }
    println!("\nNote: signature NOT verified here — use the enterprise binary.");
}

fn verify(keys_path: &Path, input: &str, expected_kid: Option<&str>) {
    // 1. Load token from file or direct input
    let token = if Path::new(input).is_file() {
        let raw = match std::fs::read_to_string(input) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("❌ Cannot read file: {e}");
                std::process::exit(1);
            }
        };
        // Try parsing as .license JSON wrapper
        if let Ok(lf) = serde_json::from_str::<LicenseFile>(&raw) {
            if lf.product != "dwarpala" {
                eprintln!("❌ Not a Dwarpala license file (product: {})", lf.product);
                std::process::exit(1);
            }
            lf.token
        } else {
            // Fallback: treat file content as raw token
            raw.trim().to_string()
        }
    } else {
        input.to_string()
    };

    // 2. Parse token format: DW.<hdr>.<claims>.<sig>
    let parts: Vec<&str> = token.trim().splitn(4, '.').collect();
    if parts.len() != 4 || parts[0] != "DW" {
        eprintln!("❌ Invalid token format. Expected: DW.<hdr>.<claims>.<sig>");
        std::process::exit(1);
    }
    let (hdr_b64, claims_b64, sig_b64) = (parts[1], parts[2], parts[3]);

    // 3. Decode & parse header
    let hdr: TokenHeader = match B64.decode(hdr_b64)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
    {
        Some(h) => h,
        None => {
            eprintln!("❌ Failed to decode/parse header");
            std::process::exit(1);
        }
    };
    
    if hdr.alg != "EdDSA" {
        eprintln!("❌ Unsupported algorithm: {} (expected EdDSA)", hdr.alg);
        std::process::exit(1);
    }

    // 4. Resolve public key
    let store = keystore::load(keys_path).unwrap_or_else(|e| die(&e));
    let kid = expected_kid.unwrap_or(&hdr.kid);
    let entry = store.get(kid).unwrap_or_else(|| die(&format!(
        "Unknown key id \"{kid}\" (available: {})",
        store.keys.iter().map(|k| k.kid.as_str()).collect::<Vec<_>>().join(", "))));
    if entry.disable { die(&format!("key \"{kid}\" is disabled")); }
    let vk_bytes: [u8; 32] = match (from_hex32(&entry.enc), from_hex32(&entry.mask)) {
        (Some(e), Some(m)) => std::array::from_fn(|i| e[i] ^ m[i]),
        _ => die("stored enc/mask are not valid 64-character hex"),
    };
    let vk = match VerifyingKey::from_bytes(&vk_bytes) {
        Ok(k) => k,
        Err(e) => die(&format!("Invalid public key for \"{kid}\": {e}")),
    };

    // 5. Verify signature
    let message = format!("DW.{hdr_b64}.{claims_b64}");
    let sig = match B64.decode(sig_b64)
        .ok()
        .and_then(|b| <&[u8; 64]>::try_from(b.as_slice()).ok().map(|a| Signature::from_bytes(a)))
    {
        Some(s) => s,
        None => {
            eprintln!("❌ Failed to decode signature (must be 64 bytes base64url)");
            std::process::exit(1);
        }
    };

    if let Err(e) = vk.verify_strict(message.as_bytes(), &sig) {
        eprintln!("❌ Signature verification failed: {}", e);
        std::process::exit(1);
    }

    // 6. Decode & parse claims (only after sig verified)
    let claims: LicenseClaims = match B64.decode(claims_b64)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
    {
        Some(c) => c,
        None => {
            eprintln!("❌ Failed to decode/parse claims");
            std::process::exit(1);
        }
    };

    // 7. Check expiration
    let now = chrono::Utc::now().timestamp();
    let expired = claims.exp.map(|exp| now > exp).unwrap_or(false);

    // ✅ Success output
    println!("✅ License verified successfully!");
    println!();
    println!("  Key ID:      {}", hdr.kid);
    println!("  Tier:        {}", claims.tier.to_uppercase());
    println!("  Subject:     {}{}", claims.sub, 
        if claims.org.is_empty() { "".to_string() } else { format!(" ({})", claims.org) });
    println!("  Token ID:    {}", claims.jti);
    println!("  Issued:      {}", format_timestamp(claims.iat));
    println!("  Expires:     {}", 
        claims.exp.map(|e| format_timestamp(e)).unwrap_or_else(|| "perpetual".to_string()));
    if expired {
        println!("  ⚠️  Status:    EXPIRED");
    } else if let Some(exp) = claims.exp {
        let days = (exp - now) / 86400;
        println!("  Status:      Active ({} days remaining)", days);
    } else {
        println!("  Status:      Active (perpetual)");
    }
    if !claims.note.is_empty() {
        println!("  Note:        {}", claims.note);
    }
    println!();
    println!("  Token:       DW.{}.{}.<sig>", 
        hdr_b64.chars().take(20).collect::<String>(),
        claims_b64.chars().take(20).collect::<String>()
    );
}

fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| format!("({})", ts))
}

// ── Date helper ──────────────────────────────────────────────────────────────

/// Parse YYYY-MM-DD into a Unix timestamp (midnight UTC).
fn parse_date_to_unix(s: &str) -> i64 {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .expect("exp must be YYYY-MM-DD")
        .and_hms_opt(0, 0, 0).unwrap()
        .and_utc()
        .timestamp()
}