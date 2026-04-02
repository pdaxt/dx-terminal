use std::io::{self, Write};
use std::process::Command;

use anyhow::{bail, Result};

/// A model we know about and can recommend.
struct ModelEntry {
    id: &'static str,
    name: &'static str,
    size: &'static str,
    ram: &'static str,
    tool_use: u8, // 1-5 rating
    #[allow(dead_code)]
    speed: &'static str,
    description: &'static str,
    ollama_id: &'static str,
    hf_repo: &'static str,
    recommended: bool,
}

const MODEL_CATALOG: &[ModelEntry] = &[
    ModelEntry {
        id: "qwen2.5-coder-32b",
        name: "Qwen2.5 Coder 32B",
        size: "19 GB",
        ram: "24 GB",
        tool_use: 5,
        speed: "~15 tok/s",
        description: "Best local coding model. 92.7% HumanEval. Sonnet-class.",
        ollama_id: "qwen2.5-coder:32b",
        hf_repo: "Qwen/Qwen2.5-Coder-32B-Instruct-GGUF",
        recommended: true,
    },
    ModelEntry {
        id: "qwen3-coder-30b",
        name: "Qwen3 Coder 30B",
        size: "18 GB",
        ram: "24 GB",
        tool_use: 5,
        speed: "~18 tok/s",
        description: "Latest Qwen coder. 3.3B active params, agent-native.",
        ollama_id: "qwen3-coder:30b",
        hf_repo: "Qwen/Qwen3-Coder-30B-A3B-GGUF",
        recommended: false,
    },
    ModelEntry {
        id: "qwen3-8b",
        name: "Qwen3 8B",
        size: "5.2 GB",
        ram: "8 GB",
        tool_use: 5,
        speed: "~30 tok/s",
        description: "Best for 8GB RAM machines. Great tool use.",
        ollama_id: "qwen3:8b",
        hf_repo: "Qwen/Qwen3-8B-GGUF",
        recommended: false,
    },
    ModelEntry {
        id: "devstral",
        name: "Devstral Small",
        size: "15 GB",
        ram: "24 GB",
        tool_use: 5,
        speed: "~20 tok/s",
        description: "Mistral's coding specialist. Excellent tool use.",
        ollama_id: "devstral:latest",
        hf_repo: "mistralai/Devstral-Small-2505-GGUF",
        recommended: false,
    },
    ModelEntry {
        id: "qwen3-4b",
        name: "Qwen3 4B",
        size: "2.6 GB",
        ram: "4 GB",
        tool_use: 4,
        speed: "~50 tok/s",
        description: "Lightweight. Good for older machines.",
        ollama_id: "qwen3:4b",
        hf_repo: "Qwen/Qwen3-4B-GGUF",
        recommended: false,
    },
    ModelEntry {
        id: "llama3.1-8b",
        name: "Llama 3.1 8B",
        size: "4.7 GB",
        ram: "8 GB",
        tool_use: 4,
        speed: "~30 tok/s",
        description: "Meta's workhorse. Solid tool use.",
        ollama_id: "llama3.1:latest",
        hf_repo: "meta-llama/Llama-3.1-8B-Instruct-GGUF",
        recommended: false,
    },
    ModelEntry {
        id: "qwen3-1.7b",
        name: "Qwen3 1.7B",
        size: "1.1 GB",
        ram: "2 GB",
        tool_use: 3,
        speed: "~80 tok/s",
        description: "Ultra-light. Basic tool use, fast.",
        ollama_id: "qwen3:1.7b",
        hf_repo: "Qwen/Qwen3-1.7B-GGUF",
        recommended: false,
    },
];

/// Check system specs and return (total_ram_gb, free_disk_gb).
fn system_specs() -> (u64, u64) {
    let ram_gb = if let Ok(output) = Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()
            .unwrap_or(0)
            / (1024 * 1024 * 1024)
    } else {
        8 // assume 8GB
    };

    let disk_gb = if let Ok(output) = Command::new("df").args(["-g", "/"]).output() {
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .nth(1)
            .and_then(|line| line.split_whitespace().nth(3))
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(50)
    } else {
        50
    };

    (ram_gb, disk_gb)
}

/// Check if Ollama is installed.
fn ollama_installed() -> bool {
    Command::new("ollama")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check which models are already downloaded.
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

/// Interactive model setup. Returns the model name to use.
pub fn interactive_setup() -> Result<String> {
    let (ram_gb, disk_gb) = system_specs();
    let has_ollama = ollama_installed();
    let installed = if has_ollama {
        installed_models()
    } else {
        vec![]
    };

    eprintln!();
    eprintln!("  Welcome to DXOS");
    eprintln!("  ────────────────────────────────────────────");
    eprintln!("  System: {}GB RAM | {}GB free disk", ram_gb, disk_gb);
    eprintln!(
        "  Ollama: {}",
        if has_ollama { "installed" } else { "not found" }
    );
    eprintln!();

    // Check for already-installed compatible models
    let compatible: Vec<&ModelEntry> = MODEL_CATALOG
        .iter()
        .filter(|m| installed.iter().any(|i| i == m.ollama_id))
        .collect();

    if !compatible.is_empty() {
        eprintln!("  Models already installed:");
        for m in &compatible {
            let stars = "★".repeat(m.tool_use as usize);
            let empty = "☆".repeat(5 - m.tool_use as usize);
            eprintln!(
                "    {} {} — tool use: {}{}",
                m.ollama_id, m.size, stars, empty
            );
        }
        eprintln!();
    }

    // Filter catalog to models that fit
    let fits: Vec<&ModelEntry> = MODEL_CATALOG
        .iter()
        .filter(|m| {
            let size_gb: f64 = m.size.replace(" GB", "").parse().unwrap_or(99.0);
            size_gb < disk_gb as f64
        })
        .collect();

    eprintln!("  Available models (sorted by recommendation):");
    eprintln!();
    eprintln!(
        "  {:>3}  {:<22} {:>7}  {:>7}  {:<10}  {}",
        "#", "MODEL", "SIZE", "RAM", "TOOL USE", "DESCRIPTION"
    );
    eprintln!("  ───  ──────────────────────  ───────  ───────  ──────────  ────────────────────────────────────");

    for (i, m) in fits.iter().enumerate() {
        let stars = "★".repeat(m.tool_use as usize);
        let empty = "☆".repeat(5 - m.tool_use as usize);
        let installed_mark = if installed.iter().any(|im| im == m.ollama_id) {
            " ✓"
        } else {
            ""
        };
        let rec = if m.recommended {
            " ← recommended"
        } else {
            ""
        };

        eprintln!(
            "  [{:>1}]  {:<22} {:>7}  {:>7}  {}{:<5}  {}{}{}",
            i + 1,
            m.name,
            m.size,
            m.ram,
            stars,
            empty,
            m.description,
            installed_mark,
            rec
        );
    }

    eprintln!();
    eprintln!("  [0]  Skip — I'll set ANTHROPIC_API_KEY or OPENAI_API_KEY instead");
    eprintln!();

    // Prompt user
    eprint!("  Choose a model [1]: ");
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    let choice: usize = if input.is_empty() {
        1 // default: first (recommended)
    } else if let Ok(n) = input.parse::<usize>() {
        n
    } else {
        // Check if they typed a model name directly
        if let Some(m) = MODEL_CATALOG.iter().find(|m| {
            m.id == input
                || m.ollama_id == input
                || m.name.to_lowercase().contains(&input.to_lowercase())
        }) {
            return download_model(m, has_ollama);
        }
        1
    };

    if choice == 0 {
        eprintln!();
        eprintln!("  Set your key and run again:");
        eprintln!("    export ANTHROPIC_API_KEY=sk-ant-...");
        eprintln!("    export OPENAI_API_KEY=sk-...");
        bail!("No model selected. Set an API key to use a cloud provider.");
    }

    if choice > fits.len() {
        bail!("Invalid choice: {choice}");
    }

    let selected = fits[choice - 1];

    // If already installed, just return it
    if installed.iter().any(|i| i == selected.ollama_id) {
        eprintln!();
        eprintln!("  {} is already installed. Ready to go!", selected.name);
        return Ok(selected.ollama_id.to_string());
    }

    download_model(selected, has_ollama)
}

fn download_model(model: &ModelEntry, has_ollama: bool) -> Result<String> {
    if !has_ollama {
        eprintln!();
        eprintln!("  Ollama is required to run local models.");
        eprintln!("  Install it:");
        eprintln!();
        eprintln!("    curl -fsSL https://ollama.com/install.sh | sh");
        eprintln!();
        eprintln!("  Or download from: https://ollama.com/download");
        eprintln!();
        eprintln!("  After installing, run: dxos setup");
        bail!("Ollama not installed");
    }

    eprintln!();
    eprintln!("  Downloading {} ({})...", model.name, model.size);
    eprintln!("  Source: ollama.com (fastest)");
    eprintln!("  HuggingFace mirror: huggingface.co/{}", model.hf_repo);
    eprintln!();

    // Pull via Ollama
    let status = Command::new("ollama")
        .args(["pull", model.ollama_id])
        .status()?;

    if !status.success() {
        // Fallback: try HuggingFace via ollama
        eprintln!("  Ollama pull failed. Trying HuggingFace...");
        let hf_url = format!("hf.co/{}", model.hf_repo);
        let status = Command::new("ollama").args(["pull", &hf_url]).status()?;

        if !status.success() {
            bail!(
                "Failed to download {}. Try manually:\n  ollama pull {}\n  or download from huggingface.co/{}",
                model.name,
                model.ollama_id,
                model.hf_repo
            );
        }
    }

    eprintln!();
    eprintln!("  {} installed successfully!", model.name);
    eprintln!("  Run: dxos run \"your task here\"");

    Ok(model.ollama_id.to_string())
}

/// Check if any usable model exists. If not, trigger interactive setup.
#[allow(dead_code)]
pub fn ensure_model_available() -> Result<String> {
    // Check Ollama models
    if ollama_installed() {
        let installed = installed_models();
        // Prefer models with good tool use
        let preferred = ["qwen3", "llama3", "mistral", "gemma", "devstral"];
        for pref in &preferred {
            for model in &installed {
                if model.contains(pref) {
                    return Ok(model.clone());
                }
            }
        }
    }

    // No local model found — trigger setup
    interactive_setup()
}
