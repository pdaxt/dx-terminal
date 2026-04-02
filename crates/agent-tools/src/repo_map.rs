use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use dx_agent_core::Result;

// ── Public types ──

#[derive(Debug, Deserialize)]
pub struct RepoMapInput {
    /// Root directory to scan (defaults to cwd if omitted).
    pub path: Option<String>,
    /// Maximum number of files to process (default: 500).
    pub max_files: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct RepoMapOutput {
    pub map: String,
    pub files_scanned: usize,
    pub definitions_found: usize,
}

// ── Definition extraction ──

#[derive(Debug, Clone)]
struct Definition {
    kind: &'static str,
    name: String,
    line: usize,
}

/// Supported source extensions.
const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "js", "py", "go", "java", "c", "cpp", "rb", "tsx", "jsx",
];

fn is_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| SOURCE_EXTENSIONS.contains(&ext))
}

// ── Tree-sitter Rust extraction ──

fn extract_rust_definitions(source: &[u8]) -> Vec<Definition> {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser
        .set_language(&language.into())
        .expect("failed to set Rust language");

    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut defs = Vec::new();
    collect_rust_defs(tree.root_node(), source, &mut defs);
    defs
}

fn collect_rust_defs<'a>(node: tree_sitter::Node<'a>, source: &[u8], defs: &mut Vec<Definition>) {
    match node.kind() {
        "function_item" => {
            if let Some(name) = child_by_field(node, "name", source) {
                defs.push(Definition {
                    kind: "fn",
                    name,
                    line: node.start_position().row + 1,
                });
            }
        }
        "struct_item" => {
            if let Some(name) = child_by_field(node, "name", source) {
                defs.push(Definition {
                    kind: "struct",
                    name,
                    line: node.start_position().row + 1,
                });
            }
        }
        "enum_item" => {
            if let Some(name) = child_by_field(node, "name", source) {
                defs.push(Definition {
                    kind: "enum",
                    name,
                    line: node.start_position().row + 1,
                });
            }
        }
        "trait_item" => {
            if let Some(name) = child_by_field(node, "name", source) {
                defs.push(Definition {
                    kind: "trait",
                    name,
                    line: node.start_position().row + 1,
                });
            }
        }
        "impl_item" => {
            // For impl blocks, extract "impl Type" or "impl Trait for Type"
            if let Some(name) = extract_impl_name(node, source) {
                defs.push(Definition {
                    kind: "impl",
                    name,
                    line: node.start_position().row + 1,
                });
            }
        }
        _ => {}
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_rust_defs(child, source, defs);
    }
}

fn child_by_field(node: tree_sitter::Node<'_>, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|n| n.utf8_text(source).ok())
        .map(String::from)
}

fn extract_impl_name(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    // tree-sitter-rust impl_item has "type" field for the type being implemented
    let type_node = node.child_by_field_name("type")?;
    let type_name = type_node.utf8_text(source).ok()?;

    // Check for trait impl: "impl Trait for Type"
    if let Some(trait_node) = node.child_by_field_name("trait") {
        let trait_name = trait_node.utf8_text(source).ok()?;
        Some(format!("{trait_name} for {type_name}"))
    } else {
        Some(type_name.to_string())
    }
}

// ── Regex fallback for non-Rust languages ──

fn extract_definitions_regex(source: &str, ext: &str) -> Vec<Definition> {
    match ext {
        "ts" | "tsx" | "js" | "jsx" => extract_js_ts_defs(source),
        "py" => extract_python_defs(source),
        "go" => extract_go_defs(source),
        "java" => extract_java_defs(source),
        "c" | "cpp" => extract_c_defs(source),
        "rb" => extract_ruby_defs(source),
        _ => Vec::new(),
    }
}

fn extract_js_ts_defs(source: &str) -> Vec<Definition> {
    let fn_re =
        Regex::new(r"(?m)^(?:export\s+)?(?:async\s+)?function\s+(\w+)").expect("valid regex");
    let class_re =
        Regex::new(r"(?m)^(?:export\s+)?(?:abstract\s+)?class\s+(\w+)").expect("valid regex");
    let interface_re = Regex::new(r"(?m)^(?:export\s+)?interface\s+(\w+)").expect("valid regex");
    let type_re = Regex::new(r"(?m)^(?:export\s+)?type\s+(\w+)\s*=").expect("valid regex");

    let mut defs = Vec::new();
    for_each_match(&fn_re, source, "fn", &mut defs);
    for_each_match(&class_re, source, "class", &mut defs);
    for_each_match(&interface_re, source, "interface", &mut defs);
    for_each_match(&type_re, source, "type", &mut defs);
    defs.sort_by_key(|d| d.line);
    defs
}

fn extract_python_defs(source: &str) -> Vec<Definition> {
    let fn_re = Regex::new(r"(?m)^\s*(?:async\s+)?def\s+(\w+)").expect("valid regex");
    let class_re = Regex::new(r"(?m)^\s*class\s+(\w+)").expect("valid regex");

    let mut defs = Vec::new();
    for_each_match(&fn_re, source, "def", &mut defs);
    for_each_match(&class_re, source, "class", &mut defs);
    defs.sort_by_key(|d| d.line);
    defs
}

fn extract_go_defs(source: &str) -> Vec<Definition> {
    let fn_re = Regex::new(r"(?m)^func\s+(?:\([^)]+\)\s+)?(\w+)").expect("valid regex");
    let type_re = Regex::new(r"(?m)^type\s+(\w+)\s+struct").expect("valid regex");
    let iface_re = Regex::new(r"(?m)^type\s+(\w+)\s+interface").expect("valid regex");

    let mut defs = Vec::new();
    for_each_match(&fn_re, source, "func", &mut defs);
    for_each_match(&type_re, source, "struct", &mut defs);
    for_each_match(&iface_re, source, "interface", &mut defs);
    defs.sort_by_key(|d| d.line);
    defs
}

fn extract_java_defs(source: &str) -> Vec<Definition> {
    let class_re = Regex::new(
        r"(?m)^\s*(?:public|private|protected)?\s*(?:abstract\s+)?(?:class|interface|enum)\s+(\w+)",
    )
    .expect("valid regex");
    let method_re = Regex::new(
        r"(?m)^\s*(?:public|private|protected)\s+(?:static\s+)?(?:\w+(?:<[^>]*>)?)\s+(\w+)\s*\(",
    )
    .expect("valid regex");

    let mut defs = Vec::new();
    for_each_match(&class_re, source, "class", &mut defs);
    for_each_match(&method_re, source, "method", &mut defs);
    defs.sort_by_key(|d| d.line);
    defs
}

fn extract_c_defs(source: &str) -> Vec<Definition> {
    let fn_re = Regex::new(r"(?m)^(?:\w+[\s*]+)+(\w+)\s*\([^)]*\)\s*\{").expect("valid regex");
    let struct_re = Regex::new(r"(?m)^(?:typedef\s+)?struct\s+(\w+)").expect("valid regex");

    let mut defs = Vec::new();
    for_each_match(&fn_re, source, "fn", &mut defs);
    for_each_match(&struct_re, source, "struct", &mut defs);
    defs.sort_by_key(|d| d.line);
    defs
}

fn extract_ruby_defs(source: &str) -> Vec<Definition> {
    let fn_re = Regex::new(r"(?m)^\s*def\s+(\w+[!?]?)").expect("valid regex");
    let class_re = Regex::new(r"(?m)^\s*class\s+(\w+)").expect("valid regex");
    let mod_re = Regex::new(r"(?m)^\s*module\s+(\w+)").expect("valid regex");

    let mut defs = Vec::new();
    for_each_match(&fn_re, source, "def", &mut defs);
    for_each_match(&class_re, source, "class", &mut defs);
    for_each_match(&mod_re, source, "module", &mut defs);
    defs.sort_by_key(|d| d.line);
    defs
}

fn for_each_match(re: &Regex, source: &str, kind: &'static str, defs: &mut Vec<Definition>) {
    for m in re.find_iter(source) {
        let line = source[..m.start()].matches('\n').count() + 1;
        if let Some(caps) = re.captures(&source[m.start()..]) {
            if let Some(name_match) = caps.get(1) {
                defs.push(Definition {
                    kind,
                    name: name_match.as_str().to_string(),
                    line,
                });
            }
        }
    }
}

// ── Main entry point ──

/// Generate a repository map showing definitions in all source files.
pub fn repo_map(input: RepoMapInput, cwd: &Path) -> Result<RepoMapOutput> {
    let root = input.path.as_ref().map_or_else(
        || cwd.to_path_buf(),
        |p| {
            let path = PathBuf::from(p);
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        },
    );

    let max_files = input.max_files.unwrap_or(500);

    // Collect source files, respecting common ignore patterns
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            // Always allow the root entry
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            // Skip hidden dirs, target, node_modules, .git, etc.
            !name.starts_with('.')
                && name != "target"
                && name != "node_modules"
                && name != "vendor"
                && name != "__pycache__"
                && name != "dist"
                && name != "build"
        })
    {
        let Ok(entry) = entry else { continue };
        if entry.file_type().is_file() && is_source_file(entry.path()) {
            files.push(entry.into_path());
            if files.len() >= max_files {
                break;
            }
        }
    }

    files.sort();

    // Extract definitions from each file
    let mut file_defs: BTreeMap<String, Vec<Definition>> = BTreeMap::new();
    let mut total_defs = 0usize;

    for file_path in &files {
        let Ok(content) = std::fs::read(file_path) else {
            continue;
        };

        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let defs = if ext == "rs" {
            extract_rust_definitions(&content)
        } else {
            let source = String::from_utf8_lossy(&content);
            extract_definitions_regex(&source, ext)
        };

        if !defs.is_empty() {
            total_defs += defs.len();
            let rel_path = file_path
                .strip_prefix(&root)
                .unwrap_or(file_path)
                .to_string_lossy()
                .to_string();
            file_defs.insert(rel_path, defs);
        }
    }

    // Build output string
    let mut output = String::new();
    for (path, defs) in &file_defs {
        output.push_str(path);
        output.push_str(":\n");
        for def in defs {
            output.push_str(&format!("  {} {}() [{}]\n", def.kind, def.name, def.line));
        }
        output.push('\n');
    }

    Ok(RepoMapOutput {
        map: output,
        files_scanned: files.len(),
        definitions_found: total_defs,
    })
}
