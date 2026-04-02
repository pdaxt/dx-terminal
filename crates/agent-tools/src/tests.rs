#[cfg(test)]
mod tests {
    use std::fs;

    fn test_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("create temp dir");
        // Create some test files
        fs::write(
            dir.path().join("hello.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .expect("write hello.rs");
        fs::write(
            dir.path().join("lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
        )
        .expect("write lib.rs");
        fs::create_dir_all(dir.path().join("src")).expect("create src dir");
        fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").expect("write src/main.rs");
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .expect("write Cargo.toml");
        dir
    }

    // ── read_file tests ──

    #[test]
    fn read_file_returns_content_with_line_numbers() {
        let dir = test_dir();
        let input = crate::ReadInput {
            path: "hello.rs".to_string(),
            offset: None,
            limit: None,
        };
        let output = crate::read_file(input, dir.path()).expect("read file");
        assert!(output.content.contains("fn main()"));
        assert_eq!(output.total_lines, 3);
        assert_eq!(output.start_line, 1);
    }

    #[test]
    fn read_file_with_offset_and_limit() {
        let dir = test_dir();
        let input = crate::ReadInput {
            path: "hello.rs".to_string(),
            offset: Some(1),
            limit: Some(1),
        };
        let output = crate::read_file(input, dir.path()).expect("read file");
        assert_eq!(output.num_lines, 1);
        assert!(output.content.contains("println"));
        assert_eq!(output.start_line, 2);
    }

    #[test]
    fn read_file_nonexistent_returns_error() {
        let dir = test_dir();
        let input = crate::ReadInput {
            path: "nope.rs".to_string(),
            offset: None,
            limit: None,
        };
        assert!(crate::read_file(input, dir.path()).is_err());
    }

    // ── write_file tests ──

    #[test]
    fn write_file_creates_new_file() {
        let dir = test_dir();
        let input = crate::WriteInput {
            path: "new.txt".to_string(),
            content: "hello world".to_string(),
        };
        let output = crate::write_file(input, dir.path()).expect("write file");
        assert_eq!(output.bytes_written, 11);
        assert_eq!(
            fs::read_to_string(dir.path().join("new.txt")).unwrap(),
            "hello world"
        );
    }

    #[test]
    fn write_file_creates_parent_directories() {
        let dir = test_dir();
        let input = crate::WriteInput {
            path: "deep/nested/dir/file.txt".to_string(),
            content: "nested".to_string(),
        };
        let output = crate::write_file(input, dir.path()).expect("write file");
        assert_eq!(output.bytes_written, 6);
        assert!(dir.path().join("deep/nested/dir/file.txt").exists());
    }

    // ── edit_file tests ──

    #[test]
    fn edit_file_replaces_unique_string() {
        let dir = test_dir();
        let input = crate::EditInput {
            path: "hello.rs".to_string(),
            old_string: "println!(\"hello\")".to_string(),
            new_string: "println!(\"world\")".to_string(),
            replace_all: false,
        };
        let output = crate::edit_file(input, dir.path()).expect("edit file");
        assert_eq!(output.replacements, 1);
        let content = fs::read_to_string(dir.path().join("hello.rs")).unwrap();
        assert!(content.contains("println!(\"world\")"));
        assert!(!content.contains("println!(\"hello\")"));
    }

    #[test]
    fn edit_file_errors_on_non_unique_match() {
        let dir = test_dir();
        // Write a file with duplicate content
        fs::write(dir.path().join("dup.rs"), "let x = 1;\nlet y = 1;\n").unwrap();
        let input = crate::EditInput {
            path: "dup.rs".to_string(),
            old_string: "= 1".to_string(),
            new_string: "= 2".to_string(),
            replace_all: false,
        };
        let result = crate::edit_file(input, dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("2 locations"));
    }

    #[test]
    fn edit_file_replace_all_works() {
        let dir = test_dir();
        fs::write(dir.path().join("dup.rs"), "let x = 1;\nlet y = 1;\n").unwrap();
        let input = crate::EditInput {
            path: "dup.rs".to_string(),
            old_string: "= 1".to_string(),
            new_string: "= 2".to_string(),
            replace_all: true,
        };
        let output = crate::edit_file(input, dir.path()).expect("edit file");
        assert_eq!(output.replacements, 2);
        let content = fs::read_to_string(dir.path().join("dup.rs")).unwrap();
        assert!(!content.contains("= 1"));
    }

    #[test]
    fn edit_file_errors_when_old_equals_new() {
        let dir = test_dir();
        let input = crate::EditInput {
            path: "hello.rs".to_string(),
            old_string: "fn main".to_string(),
            new_string: "fn main".to_string(),
            replace_all: false,
        };
        assert!(crate::edit_file(input, dir.path()).is_err());
    }

    // ── glob tests ──

    #[test]
    fn glob_finds_files_by_pattern() {
        let dir = test_dir();
        let input = crate::GlobInput {
            pattern: "*.rs".to_string(),
            path: None,
        };
        let output = crate::glob_files(input, dir.path()).expect("glob");
        assert!(output.count >= 2); // hello.rs, lib.rs at minimum
    }

    #[test]
    fn glob_finds_nested_files() {
        let dir = test_dir();
        let input = crate::GlobInput {
            pattern: "*.rs".to_string(),
            path: Some(dir.path().join("src").to_string_lossy().to_string()),
        };
        let output = crate::glob_files(input, dir.path()).expect("glob");
        assert!(output.count >= 1);
    }

    // ── grep tests ──

    #[test]
    fn grep_finds_matching_lines() {
        let dir = test_dir();
        let input = crate::GrepInput {
            pattern: "fn main".to_string(),
            path: None,
            glob: None,
            case_insensitive: None,
            max_results: None,
        };
        let output = crate::grep_content(input, dir.path()).expect("grep");
        assert!(output.count >= 2); // hello.rs and src/main.rs
    }

    #[test]
    fn grep_respects_case_insensitive() {
        let dir = test_dir();
        let input = crate::GrepInput {
            pattern: "FN MAIN".to_string(),
            path: None,
            glob: None,
            case_insensitive: Some(true),
            max_results: None,
        };
        let output = crate::grep_content(input, dir.path()).expect("grep");
        assert!(output.count >= 2);
    }

    #[test]
    fn grep_respects_glob_filter() {
        let dir = test_dir();
        let input = crate::GrepInput {
            pattern: "fn main".to_string(),
            path: None,
            glob: Some("hello.*".to_string()),
            case_insensitive: None,
            max_results: None,
        };
        let output = crate::grep_content(input, dir.path()).expect("grep");
        assert_eq!(output.count, 1);
    }

    #[test]
    fn grep_respects_max_results() {
        let dir = test_dir();
        let input = crate::GrepInput {
            pattern: "fn".to_string(),
            path: None,
            glob: None,
            case_insensitive: None,
            max_results: Some(1),
        };
        let output = crate::grep_content(input, dir.path()).expect("grep");
        assert_eq!(output.count, 1);
        assert!(output.truncated);
    }

    // ── bash tests ──

    #[test]
    fn bash_executes_simple_command() {
        let dir = test_dir();
        let input = crate::BashInput {
            command: "echo hello".to_string(),
            timeout: None,
            description: None,
        };
        let output = crate::execute_bash(input, dir.path()).expect("bash");
        assert_eq!(output.stdout.trim(), "hello");
        assert_eq!(output.exit_code, Some(0));
        assert!(!output.timed_out);
    }

    #[test]
    fn bash_captures_stderr() {
        let dir = test_dir();
        let input = crate::BashInput {
            command: "echo err >&2".to_string(),
            timeout: None,
            description: None,
        };
        let output = crate::execute_bash(input, dir.path()).expect("bash");
        assert!(output.stderr.contains("err"));
    }

    #[test]
    fn bash_returns_nonzero_exit_code() {
        let dir = test_dir();
        let input = crate::BashInput {
            command: "exit 42".to_string(),
            timeout: None,
            description: None,
        };
        let output = crate::execute_bash(input, dir.path()).expect("bash");
        assert_eq!(output.exit_code, Some(42));
    }

    #[test]
    fn bash_times_out() {
        let dir = test_dir();
        let input = crate::BashInput {
            command: "sleep 10".to_string(),
            timeout: Some(100), // 100ms timeout
            description: None,
        };
        let output = crate::execute_bash(input, dir.path()).expect("bash");
        assert!(output.timed_out);
    }

    // ── execute_tool dispatch tests ──

    #[test]
    fn execute_tool_routes_to_read_file() {
        let dir = test_dir();
        let input = r#"{"path": "hello.rs"}"#;
        let result = crate::execute_tool("read_file", input, dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().contains("fn main"));
    }

    #[test]
    fn execute_tool_routes_to_glob() {
        let dir = test_dir();
        let input = r#"{"pattern": "*.rs"}"#;
        let result = crate::execute_tool("glob", input, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn execute_tool_returns_error_for_unknown_tool() {
        let dir = test_dir();
        let result = crate::execute_tool("nonexistent", "{}", dir.path());
        assert!(result.is_err());
    }

    // ── repo_map tests ──

    #[test]
    fn repo_map_extracts_rust_functions() {
        let dir = test_dir();
        let input = crate::RepoMapInput {
            path: None,
            max_files: None,
        };
        let output = crate::repo_map(input, dir.path()).expect("repo_map");
        // hello.rs has fn main, lib.rs has fn add
        assert!(output.map.contains("fn main()"));
        assert!(output.map.contains("fn add()"));
        assert!(output.definitions_found >= 2);
    }

    #[test]
    fn repo_map_extracts_rust_structs_and_enums() {
        let dir = test_dir();
        fs::write(
            dir.path().join("types.rs"),
            "pub struct Config {\n    pub name: String,\n}\n\npub enum Mode {\n    Fast,\n    Slow,\n}\n",
        )
        .expect("write types.rs");

        let input = crate::RepoMapInput {
            path: None,
            max_files: None,
        };
        let output = crate::repo_map(input, dir.path()).expect("repo_map");
        assert!(output.map.contains("struct Config()"));
        assert!(output.map.contains("enum Mode()"));
    }

    #[test]
    fn repo_map_extracts_rust_impl_and_trait() {
        let dir = test_dir();
        fs::write(
            dir.path().join("traits.rs"),
            "pub trait Runnable {\n    fn run(&self);\n}\n\nstruct App;\n\nimpl Runnable for App {\n    fn run(&self) {}\n}\n",
        )
        .expect("write traits.rs");

        let input = crate::RepoMapInput {
            path: None,
            max_files: None,
        };
        let output = crate::repo_map(input, dir.path()).expect("repo_map");
        assert!(output.map.contains("trait Runnable()"));
        assert!(output.map.contains("impl Runnable for App()"));
    }

    #[test]
    fn repo_map_handles_python_files() {
        let dir = test_dir();
        fs::write(
            dir.path().join("app.py"),
            "class Server:\n    def start(self):\n        pass\n\ndef main():\n    pass\n",
        )
        .expect("write app.py");

        let input = crate::RepoMapInput {
            path: None,
            max_files: None,
        };
        let output = crate::repo_map(input, dir.path()).expect("repo_map");
        assert!(output.map.contains("class Server()"));
        assert!(output.map.contains("def main()"));
    }

    #[test]
    fn repo_map_handles_typescript_files() {
        let dir = test_dir();
        fs::write(
            dir.path().join("index.ts"),
            "export function fetchData() {\n  return null;\n}\n\nexport interface Config {\n  url: string;\n}\n\nexport class App {\n}\n",
        )
        .expect("write index.ts");

        let input = crate::RepoMapInput {
            path: None,
            max_files: None,
        };
        let output = crate::repo_map(input, dir.path()).expect("repo_map");
        assert!(output.map.contains("fn fetchData()"));
        assert!(output.map.contains("interface Config()"));
        assert!(output.map.contains("class App()"));
    }

    #[test]
    fn repo_map_respects_max_files() {
        let dir = test_dir();
        let input = crate::RepoMapInput {
            path: None,
            max_files: Some(1),
        };
        let output = crate::repo_map(input, dir.path()).expect("repo_map");
        assert!(output.files_scanned <= 1);
    }

    #[test]
    fn repo_map_shows_line_numbers() {
        let dir = test_dir();
        let input = crate::RepoMapInput {
            path: None,
            max_files: None,
        };
        let output = crate::repo_map(input, dir.path()).expect("repo_map");
        // Line numbers should appear in brackets
        assert!(output.map.contains("[1]"));
    }

    #[test]
    fn repo_map_via_execute_tool() {
        let dir = test_dir();
        let input = r#"{}"#;
        let result = crate::execute_tool("repo_map", input, dir.path());
        assert!(result.is_ok());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(json["definitions_found"].as_u64().unwrap() >= 2);
    }
}
