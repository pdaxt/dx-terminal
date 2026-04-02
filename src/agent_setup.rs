use std::process::Command;

use anyhow::{bail, Result};

/// Hardware profile detected from the system.
pub struct HardwareProfile {
    pub ram_gb: u64,
    pub os: &'static str,
    pub arch: &'static str,
}

/// Model recommendation based on hardware.
struct ModelRec {
    id: &'static str,
    size: &'static str,
}

impl HardwareProfile {
    pub fn detect() -> Self {
        let ram_gb = detect_ram_gb();
        Self {
            ram_gb,
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        }
    }

    fn recommended_model(&self) -> ModelRec {
        if self.ram_gb >= 32 {
            ModelRec {
                id: "qwen2.5-coder:32b",
                size: "19GB",
            }
        } else if self.ram_gb >= 16 {
            ModelRec {
                id: "qwen2.5-coder:14b",
                size: "9GB",
            }
        } else if self.ram_gb >= 8 {
            ModelRec {
                id: "qwen3:8b",
                size: "5.2GB",
            }
        } else {
            ModelRec {
                id: "qwen3:1.7b",
                size: "1.1GB",
            }
        }
    }
}

fn detect_ram_gb() -> u64 {
    if cfg!(target_os = "macos") {
        if let Ok(output) = Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
            if let Ok(bytes) = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u64>()
            {
                return bytes / (1024 * 1024 * 1024);
            }
        }
    } else if cfg!(target_os = "linux") {
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            if let Some(line) = content.lines().find(|l| l.starts_with("MemTotal")) {
                if let Some(kb) = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u64>().ok())
                {
                    return kb / (1024 * 1024);
                }
            }
        }
    }
    8 // conservative default
}

/// Check if Ollama is installed and running.
fn ollama_is_installed() -> bool {
    Command::new("ollama")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if Ollama server is responding.
fn ollama_is_running() -> bool {
    reqwest::blocking::Client::new()
        .get("http://127.0.0.1:11434/api/version")
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Get list of installed models.
fn installed_models() -> Vec<String> {
    Command::new("ollama")
        .arg("list")
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .skip(1)
                .filter_map(|l| l.split_whitespace().next())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Check if a compatible coding model is already installed.
fn find_compatible_model(models: &[String]) -> Option<String> {
    let preferred = [
        "qwen2.5-coder:32b",
        "qwen2.5-coder:14b",
        "qwen2.5-coder",
        "qwen3-coder",
        "devstral",
        "qwen3:8b",
        "qwen3:4b",
        "qwen3",
        "llama3.1",
        "mistral",
    ];
    for pref in &preferred {
        for model in models {
            if model.contains(pref) {
                return Some(model.clone());
            }
        }
    }
    None
}

/// Install Ollama automatically.
fn install_ollama() -> Result<()> {
    eprintln!("  Installing Ollama...");

    if cfg!(target_os = "macos") {
        // Try brew first (fastest, no sudo)
        let status = Command::new("brew").args(["install", "ollama"]).status();

        if let Ok(s) = status {
            if s.success() {
                eprintln!("  Ollama installed via Homebrew.");
                return Ok(());
            }
        }

        // Fallback: direct download
        let status = Command::new("sh")
            .args(["-c", "curl -fsSL https://ollama.com/install.sh | sh"])
            .status()?;

        if !status.success() {
            bail!("Failed to install Ollama. Install manually: https://ollama.com/download");
        }
    } else if cfg!(target_os = "linux") {
        let status = Command::new("sh")
            .args(["-c", "curl -fsSL https://ollama.com/install.sh | sh"])
            .status()?;

        if !status.success() {
            bail!("Failed to install Ollama. Install manually: https://ollama.com/download");
        }
    } else {
        bail!("Auto-install not supported on {}. Install Ollama manually: https://ollama.com/download", std::env::consts::OS);
    }

    eprintln!("  Ollama installed successfully.");
    Ok(())
}

/// Start Ollama server if not running.
fn ensure_ollama_running() -> Result<()> {
    if ollama_is_running() {
        return Ok(());
    }

    eprintln!("  Starting Ollama...");

    // Start in background
    Command::new("ollama")
        .arg("serve")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    // Wait for it to be ready
    for i in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if ollama_is_running() {
            return Ok(());
        }
        if i % 4 == 0 {
            eprint!(".");
        }
    }

    bail!("Ollama server didn't start in time. Try: ollama serve");
}

/// Pull a model with progress display.
fn pull_model(model_id: &str, size: &str) -> Result<()> {
    eprintln!("  Downloading {model_id} ({size})...");
    eprintln!("  This is a one-time download. Future runs start instantly.");
    eprintln!();

    let status = Command::new("ollama").args(["pull", model_id]).status()?;

    if !status.success() {
        bail!("Failed to download {model_id}. Check your internet connection and try: ollama pull {model_id}");
    }

    eprintln!();
    eprintln!("  Model ready.");
    Ok(())
}

/// The main auto-setup flow. Called on first run or when no model is available.
/// Returns the model ID to use.
pub fn auto_setup() -> Result<String> {
    let hw = HardwareProfile::detect();

    eprintln!();
    eprintln!("  \x1b[1;36mdxos\x1b[0m first-time setup");
    eprintln!(
        "  \x1b[2m{}GB RAM | {} | {}\x1b[0m",
        hw.ram_gb, hw.os, hw.arch
    );
    eprintln!();

    // Step 1: Ensure Ollama is installed
    if !ollama_is_installed() {
        eprintln!("  Ollama not found. Installing automatically...");
        install_ollama()?;
    }

    // Step 2: Ensure Ollama is running
    ensure_ollama_running()?;

    // Step 3: Check for existing compatible models
    let models = installed_models();
    if let Some(model) = find_compatible_model(&models) {
        eprintln!("  Found model: {model}");
        eprintln!("  \x1b[32mReady.\x1b[0m");
        eprintln!();
        return Ok(model);
    }

    // Step 4: Download the best model for this hardware
    let rec = hw.recommended_model();
    eprintln!("  No coding model found. Selecting best for your hardware...");
    pull_model(rec.id, rec.size)?;

    eprintln!("  \x1b[32mReady. Run `dxos` anytime — no setup needed again.\x1b[0m");
    eprintln!();

    Ok(rec.id.to_string())
}

/// Quick check: is everything ready? Returns model ID if yes, triggers setup if no.
pub fn ensure_ready() -> Result<String> {
    // Fast path: Ollama running + compatible model exists
    if ollama_is_running() {
        let models = installed_models();
        if let Some(model) = find_compatible_model(&models) {
            return Ok(model);
        }
    }

    // Slow path: need setup
    auto_setup()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_detection_returns_nonzero() {
        let hw = HardwareProfile::detect();
        assert!(hw.ram_gb > 0);
    }

    #[test]
    fn model_recommendation_scales_with_ram() {
        let low = HardwareProfile {
            ram_gb: 4,
            os: "macos",
            arch: "aarch64",
        };
        let mid = HardwareProfile {
            ram_gb: 16,
            os: "macos",
            arch: "aarch64",
        };
        let high = HardwareProfile {
            ram_gb: 32,
            os: "macos",
            arch: "aarch64",
        };

        assert!(low.recommended_model().id.contains("1.7b"));
        assert!(mid.recommended_model().id.contains("14b"));
        assert!(high.recommended_model().id.contains("32b"));
    }
}
