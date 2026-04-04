use clap::Subcommand;
use colored::Colorize;
use sha2::{Digest, Sha256, Sha512};
use md5::Md5;

use crate::display;

#[derive(Subcommand)]
pub enum HashAction {
    /// MD5 hash of text or file
    Md5 {
        /// Text to hash (or use --file)
        text: Option<String>,
        /// Hash a file instead
        #[arg(short, long)]
        file: Option<String>,
    },
    /// SHA-256 hash of text or file
    Sha256 {
        text: Option<String>,
        #[arg(short, long)]
        file: Option<String>,
    },
    /// SHA-512 hash of text or file
    Sha512 {
        text: Option<String>,
        #[arg(short, long)]
        file: Option<String>,
    },
    /// Encode text (base64, hex, url)
    Encode {
        /// Encoding: base64, hex, url
        encoding: String,
        /// Text to encode
        text: String,
    },
    /// Decode text (base64, hex, url)
    Decode {
        /// Encoding: base64, hex
        encoding: String,
        /// Text to decode
        text: String,
    },
    /// Generate a secure random password
    #[command(alias = "pw")]
    Password {
        /// Length (default: 20)
        #[arg(short, long, default_value = "20")]
        length: usize,
        /// Include special characters
        #[arg(short, long)]
        special: bool,
        /// Number of passwords to generate (default: 1)
        #[arg(short, long, default_value = "1")]
        count: usize,
    },
}

pub async fn run(action: HashAction) -> Result<(), String> {
    match action {
        HashAction::Md5 { text, file } => hash_it("MD5", text, file),
        HashAction::Sha256 { text, file } => hash_it("SHA-256", text, file),
        HashAction::Sha512 { text, file } => hash_it("SHA-512", text, file),
        HashAction::Encode { encoding, text } => encode(&encoding, &text),
        HashAction::Decode { encoding, text } => decode(&encoding, &text),
        HashAction::Password { length, special, count } => password(length, special, count),
    }
}

fn hash_it(algo: &str, text: Option<String>, file: Option<String>) -> Result<(), String> {
    let data = if let Some(path) = file {
        std::fs::read(&path).map_err(|e| format!("read {}: {}", path, e))?
    } else if let Some(t) = text {
        t.into_bytes()
    } else {
        return Err("Provide text or --file".into());
    };

    let hash = match algo {
        "MD5" => {
            let mut hasher = Md5::new();
            hasher.update(&data);
            format!("{:x}", hasher.finalize())
        }
        "SHA-256" => {
            let mut hasher = Sha256::new();
            hasher.update(&data);
            format!("{:x}", hasher.finalize())
        }
        "SHA-512" => {
            let mut hasher = Sha512::new();
            hasher.update(&data);
            format!("{:x}", hasher.finalize())
        }
        _ => return Err(format!("Unknown algo: {}", algo)),
    };

    println!();
    display::kv("Algorithm", algo);
    display::kv("Input", &if data.len() > 50 {
        format!("({} bytes)", data.len())
    } else {
        String::from_utf8_lossy(&data).to_string()
    });
    display::kv("Hash", &hash.green().to_string());
    println!();
    Ok(())
}

fn encode(encoding: &str, text: &str) -> Result<(), String> {
    let result = match encoding.to_lowercase().as_str() {
        "base64" | "b64" => {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(text.as_bytes())
        }
        "hex" => text.bytes().map(|b| format!("{:02x}", b)).collect::<String>(),
        "url" => urlenc(text),
        _ => return Err(format!("Unknown encoding: {}. Use: base64, hex, url", encoding)),
    };

    println!();
    display::kv("Encoding", encoding);
    display::kv("Input", text);
    display::kv("Output", &result.green().to_string());
    println!();
    Ok(())
}

fn decode(encoding: &str, text: &str) -> Result<(), String> {
    let result = match encoding.to_lowercase().as_str() {
        "base64" | "b64" => {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(text.as_bytes())
                .map_err(|e| format!("base64 decode error: {}", e))?;
            String::from_utf8(bytes).map_err(|e| format!("not valid UTF-8: {}", e))?
        }
        "hex" => {
            let bytes: Result<Vec<u8>, _> = (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&text[i..i + 2], 16))
                .collect();
            let bytes = bytes.map_err(|e| format!("hex decode error: {}", e))?;
            String::from_utf8(bytes).map_err(|e| format!("not valid UTF-8: {}", e))?
        }
        _ => return Err(format!("Unknown encoding: {}. Use: base64, hex", encoding)),
    };

    println!();
    display::kv("Encoding", encoding);
    display::kv("Input", text);
    display::kv("Output", &result.green().to_string());
    println!();
    Ok(())
}

fn password(length: usize, special: bool, count: usize) -> Result<(), String> {
    let lower = "abcdefghijklmnopqrstuvwxyz";
    let upper = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let digits = "0123456789";
    let specials = "!@#$%^&*()-_=+[]{}|;:,.<>?";

    let mut charset = String::new();
    charset.push_str(lower);
    charset.push_str(upper);
    charset.push_str(digits);
    if special {
        charset.push_str(specials);
    }

    let chars: Vec<char> = charset.chars().collect();

    println!();
    for i in 0..count {
        let pw: String = (0..length)
            .map(|_| {
                let idx = random_index(chars.len());
                chars[idx]
            })
            .collect();

        // Calculate strength
        let entropy = (length as f64) * (chars.len() as f64).log2();
        let strength = if entropy > 100.0 {
            "STRONG".green().bold()
        } else if entropy > 60.0 {
            "GOOD".yellow().bold()
        } else {
            "WEAK".red().bold()
        };

        if count == 1 {
            display::kv("Password", &pw.green().bold().to_string());
            display::kv("Length", &length.to_string());
            display::kv("Charset", &chars.len().to_string());
            display::kv("Entropy", &format!("{:.0} bits", entropy));
            display::kv("Strength", &strength.to_string());
        } else {
            println!("  {:>2}  {} ({})", (i + 1).to_string().dimmed(), pw.green(), strength);
        }
    }
    println!();
    Ok(())
}

fn random_index(max: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Use system time + thread-local counter for randomness (no external crate)
    thread_local! {
        static COUNTER: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }
    COUNTER.with(|c| {
        let count = c.get().wrapping_add(1);
        c.set(count);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos() as u64;
        let hash = nanos.wrapping_mul(6364136223846793005).wrapping_add(count.wrapping_mul(1442695040888963407));
        (hash as usize) % max
    })
}

fn urlenc(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        _ => format!("%{:02X}", b),
    }).collect()
}
