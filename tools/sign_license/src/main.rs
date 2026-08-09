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
    display: serde_json::Value,
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Keygen  { kid }                                        => keygen(&kid),
        Cmd::Sign    { key, kid, tier, sub, org, exp, note, out }  =>
            sign(&key, &kid, &tier, &sub, &org, exp.as_deref(), note.as_deref(), out.as_deref()),
        Cmd::Verify  { input, kid }                                => verify(&input, kid.as_deref()),  // ← ADD THIS
        Cmd::Inspect { input }                                      => inspect(&input),
    }
}

#[derive(Parser)]
#[command(name = "sign_license", about = "Dwarpala license signing tool")]
struct Cli {
    #[command(subcommand)] cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate a new Ed25519 keypair
    Keygen {
        #[arg(long, default_value = "k1")] kid: String,
    },
    /// Sign and issue a new license
    Sign {
        /// Base64 private key (seed) from keygen
        #[arg(long)] key:  String,
        #[arg(long, default_value = "k1")] kid: String,
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
    /// Decode and display claims from a token or .license file (no sig check)
    Verify {
        /// Token string or path to .license file
        input: String,
        #[arg(long)] kid: Option<String>,
    },
    Inspect { input: String },
}

// ── keygen ───────────────────────────────────────────────────────────────────

fn keygen(kid: &str) {
    let sk   = SigningKey::generate(&mut OsRng);
    let pub_ = sk.verifying_key().to_bytes();
    let priv_ = sk.to_bytes(); // 32-byte seed

    // Random mask so enc ^ mask = pub_
    let mut mask = [0u8; 32];
    OsRng.fill_bytes(&mut mask);
    let enc: [u8; 32] = std::array::from_fn(|i| pub_[i] ^ mask[i]);

    println!("═══════════════════════════════════════════════════════════");
    println!(" Dwarpala keygen — kid: {kid}");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("🔐 PRIVATE KEY  (keep secret — pass to `sign --key`)");
    println!("   {}", B64.encode(priv_));
    println!();
    println!("📋 Paste this ObfKey{{}} block into PUBKEYS in enterprise/src/license.rs:");
    println!();
    println!("    ObfKey {{");
    println!("        kid:  \"{kid}\",");
    print_byte_array("        enc", &enc);
    print_byte_array("        mask", &mask);
    println!("    }},");
    println!();
    println!("═══════════════════════════════════════════════════════════");
}

struct ObfKey { kid: &'static str, enc: [u8; 32], mask: [u8; 32] }

impl ObfKey {
    #[inline(never)]
    fn resolve(&self) -> [u8; 32] {
        let mut k = [0u8; 32];
        for i in 0..32 {
            k[i] = std::hint::black_box(self.enc[i]) ^ self.mask[i];
        }
        k
    }
}

static PUBKEYS: &[ObfKey] = &[
    ObfKey {
        kid:  "k1",
        enc:  [
            0x1d, 0xf8, 0x65, 0x6c, 0xca, 0x29, 0x32, 0xe7,
            0xfb, 0x27, 0xf9, 0x91, 0x3e, 0x56, 0x9f, 0xa4,
            0xa7, 0xf4, 0xf8, 0x96, 0x46, 0x04, 0x3b, 0x5f,
            0xed, 0xcb, 0x55, 0x5a, 0x93, 0xf2, 0x6a, 0xd5,
        ],
        mask:  [
            0xfd, 0xcf, 0x43, 0x29, 0x5b, 0xfe, 0x56, 0xe3,
            0xe8, 0x9f, 0x1e, 0x99, 0x61, 0x1b, 0x8e, 0xae,
            0x56, 0xf9, 0xe2, 0x91, 0x9f, 0x68, 0x33, 0x6f,
            0x37, 0xee, 0x4a, 0x76, 0x65, 0x97, 0x52, 0xdd,
        ],
    },
];

fn print_byte_array(label: &str, b: &[u8; 32]) {
    println!("{label}:  [");
    for chunk in b.chunks(8) {
        print!("            ");
        for byte in chunk { print!("0x{byte:02x}, "); }
        println!();
    }
    println!("        ],");
}

// ── sign ─────────────────────────────────────────────────────────────────────

fn sign(
    key_b64: &str, kid: &str, tier: &str, sub: &str, org: &str,
    exp: Option<&str>, note: Option<&str>, out: Option<&str>,
) {
    // Load private key
    let key_seed = B64.decode(key_b64.trim())
        .expect("Invalid base64 private key");
    let key_arr: [u8; 32] = key_seed.as_slice().try_into()
        .expect("Private key must be 32 bytes");
    let sk = SigningKey::from_bytes(&key_arr);

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
    let token = if std::path::Path::new(input).is_file() {
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

fn verify(input: &str, expected_kid: Option<&str>) {
    // 1. Load token from file or direct input
    let token = if std::path::Path::new(input).is_file() {
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
    let kid = expected_kid.unwrap_or(&hdr.kid);
    let obf = match PUBKEYS.iter().find(|k| k.kid == kid) {
        Some(k) => k,
        None => {
            eprintln!("❌ Unknown key id: \"{}\" (available: {})", 
                kid, 
                PUBKEYS.iter().map(|k| k.kid).collect::<Vec<_>>().join(", ")
            );
            std::process::exit(1);
        }
    };
    
    let vk_bytes = obf.resolve();
    let vk = match VerifyingKey::from_bytes(&vk_bytes) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("❌ Invalid public key for kid \"{}\": {}", kid, e);
            std::process::exit(1);
        }
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
    let p: Vec<i64> = s.split('-')
        .map(|x| x.parse::<i64>().expect("Invalid date component"))
        .collect();
    assert_eq!(p.len(), 3, "exp must be YYYY-MM-DD");
    let (y, m, d) = (p[0], p[1], p[2]);
    // Days from Julian Day Number to Unix epoch (1970-01-01 = JDN 2440588)
    let (y, m) = if m <= 2 { (y - 1, m + 12) } else { (y, m) };
    let c  = y / 100;
    let jd = (365.25 * (y + 4716) as f64) as i64
           + (30.6001 * (m + 1) as f64) as i64
           + d - 1524;
    (jd - 2_440_588) * 86_400
}