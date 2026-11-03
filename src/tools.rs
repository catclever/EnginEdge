use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use std::env;
use std::fs;
use ignore::WalkBuilder;

use crate::protocol::*;
use crate::ast;
use crate::bridge::CloudBridge;

// --- AST Tools ---

pub fn get_outline_tool_def() -> Tool {
    Tool {
        name: "get_outline".to_string(),
        description: Some("Extracts high-level symbols (functions, structs, etc) from a file".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string"
                }
            },
            "required": ["file_path"]
        }),
    }
}

pub fn execute_get_outline(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    if let Some(args_obj) = args {
        if let Some(file_path) = args_obj.get("file_path").and_then(|m| m.as_str()) {
            match ast::get_outline(file_path) {
                Ok(outline) => {
                    return Ok(CallToolResult {
                        content: vec![Content::Text {
                            text: outline.to_string(),
                        }],
                        is_error: None,
                    });
                }
                Err(e) => {
                    return Err(JsonRpcError {
                        code: -32603,
                        message: format!("Failed to get outline: {}", e),
                        data: None,
                    });
                }
            }
        }
    }

    Err(JsonRpcError {
        code: -32602,
        message: "Missing required argument 'file_path'".to_string(),
        data: None,
    })
}

pub fn read_symbol_tool_def() -> Tool {
    Tool {
        name: "read_symbol".to_string(),
        description: Some("Extracts the exact code block for a specific target symbol".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string"
                },
                "symbol_path": {
                    "type": "string"
                }
            },
            "required": ["file_path", "symbol_path"]
        }),
    }
}

pub fn execute_read_symbol(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    if let Some(args_obj) = args {
        let file_path = args_obj.get("file_path").and_then(|m| m.as_str());
        let symbol_path = args_obj.get("symbol_path").and_then(|m| m.as_str());

        if let (Some(fp), Some(sp)) = (file_path, symbol_path) {
            match ast::read_symbol(fp, sp) {
                Ok(symbol) => {
                    return Ok(CallToolResult {
                        content: vec![Content::Text {
                            text: symbol.to_string(),
                        }],
                        is_error: None,
                    });
                }
                Err(e) => {
                    return Err(JsonRpcError {
                        code: -32603,
                        message: format!("Failed to read symbol: {}", e),
                        data: None,
                    });
                }
            }
        }
    }

    Err(JsonRpcError {
        code: -32602,
        message: "Missing required arguments 'file_path' and 'symbol_path'".to_string(),
        data: None,
    })
}

pub fn replace_symbol_tool_def() -> Tool {
    Tool {
        name: "replace_symbol".to_string(),
        description: Some("Atomically replaces the target AST node's span with new_code".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string"
                },
                "symbol_path": {
                    "type": "string"
                },
                "new_code": {
                    "type": "string"
                }
            },
            "required": ["file_path", "symbol_path", "new_code"]
        }),
    }
}

pub fn execute_replace_symbol(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    if let Some(args_obj) = args {
        let file_path = args_obj.get("file_path").and_then(|m| m.as_str());
        let symbol_path = args_obj.get("symbol_path").and_then(|m| m.as_str());
        let new_code = args_obj.get("new_code").and_then(|m| m.as_str());

        if let (Some(fp), Some(sp), Some(nc)) = (file_path, symbol_path, new_code) {
            match ast::replace_symbol(fp, sp, nc) {
                Ok(result) => {
                    return Ok(CallToolResult {
                        content: vec![Content::Text {
                            text: result.to_string(),
                        }],
                        is_error: None,
                    });
                }
                Err(e) => {
                    return Err(JsonRpcError {
                        code: -32603,
                        message: format!("Failed to replace symbol: {}", e),
                        data: None,
                    });
                }
            }
        }
    }

    Err(JsonRpcError {
        code: -32602,
        message: "Missing required arguments 'file_path', 'symbol_path', 'new_code'".to_string(),
        data: None,
    })
}

pub fn manage_imports_tool_def() -> Tool {
    Tool {
        name: "manage_imports".to_string(),
        description: Some("AST-aware import and reference manager across Rust, Go, Python, and TypeScript. Supports listing imports, idempotent adding/merging, and pruning dead/unused imports.".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the code file"
                },
                "action": {
                    "type": "string",
                    "enum": ["list", "add", "prune"],
                    "description": "Action to perform: 'list' (view all imports), 'add' (insert or merge import), 'prune' (detect and optionally remove unused imports)"
                },
                "statement": {
                    "type": "string",
                    "description": "The import statement to add (e.g. 'use std::collections::HashMap;' or 'from collections import defaultdict'). Required for action 'add'."
                },
                "auto_remove": {
                    "type": "boolean",
                    "description": "For action 'prune', whether to automatically remove detected unused imports (default false, reporting only)"
                }
            },
            "required": ["file_path", "action"]
        }),
    }
}

pub fn execute_manage_imports(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    let args_obj = args.ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing arguments".to_string(),
        data: None,
    })?;

    let file_path = args_obj.get("file_path").and_then(|f| f.as_str()).ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing required argument 'file_path'".to_string(),
        data: None,
    })?;

    let action = args_obj.get("action").and_then(|a| a.as_str()).unwrap_or("list");
    let statement = args_obj.get("statement").and_then(|s| s.as_str());
    let auto_remove = args_obj.get("auto_remove").and_then(|r| r.as_bool());

    match ast::manage_imports(file_path, action, statement, auto_remove) {
        Ok(result) => Ok(CallToolResult {
            content: vec![Content::Text {
                text: result.to_string(),
            }],
            is_error: None,
        }),
        Err(e) => Err(JsonRpcError {
            code: -32603,
            message: format!("Failed to manage imports: {}", e),
            data: None,
        }),
    }
}

// --- New Local Tools (list_files, grep_search, write_file, run_verification) ---

pub fn list_files_tool_def() -> Tool {
    Tool {
        name: "list_files".to_string(),
        description: Some("Lists files recursively in the workspace, respecting security jail and ignoring VCS/build artifacts".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Optional relative directory path to list. Defaults to workspace root."
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Optional maximum traversal depth (default 10)"
                }
            }
        }),
    }
}

pub fn execute_list_files(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    let cwd = env::current_dir().map_err(|e| JsonRpcError {
        code: -32603,
        message: format!("Failed to get current directory: {}", e),
        data: None,
    })?;

    let relative_path = args.as_ref()
        .and_then(|a| a.get("path"))
        .and_then(|p| p.as_str())
        .unwrap_or(".");

    let max_depth = args.as_ref()
        .and_then(|a| a.get("max_depth"))
        .and_then(|d| d.as_u64())
        .unwrap_or(10) as usize;

    let target_dir = cwd.join(relative_path);
    let validated_dir = crate::security::validate_path(&target_dir).map_err(|e| JsonRpcError {
        code: -32602,
        message: format!("Security validation failed: {}", e),
        data: None,
    })?;

    let mut files = Vec::new();
    let walker = build_ignore_walker(&validated_dir, Some(max_depth));
    for entry in walker.build().flatten() {
        if entry.file_type().is_some_and(|ft| ft.is_file()) && crate::security::validate_path(entry.path()).is_ok() {
            if let Ok(rel) = entry.path().strip_prefix(&cwd) {
                files.push(rel.to_string_lossy().to_string());
            }
        }
    }
    files.sort();

    Ok(CallToolResult {
        content: vec![Content::Text {
            text: json!({ "files": files }).to_string(),
        }],
        is_error: None,
    })
}

fn build_ignore_walker(target_dir: &std::path::Path, max_depth: Option<usize>) -> WalkBuilder {
    let mut builder = WalkBuilder::new(target_dir);
    builder.hidden(true); // Ignore hidden files like .git
    builder.git_ignore(true); // Respect .gitignore
    builder.git_global(true);
    builder.git_exclude(true);
    builder.ignore(true); // Respect .ignore
    if let Some(depth) = max_depth {
        builder.max_depth(Some(depth));
    }
    builder
}

pub fn grep_search_tool_def() -> Tool {
    Tool {
        name: "grep_search".to_string(),
        description: Some("Fast regex and text search across workspace files powered by ripgrep engine, respecting .gitignore and security jail".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Regex or text pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Optional subpath or file to restrict search"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matched lines to return (default: 50, max: 200)"
                }
            },
            "required": ["query"]
        }),
    }
}

pub fn execute_grep_search(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    let args_obj = args.ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing arguments".to_string(),
        data: None,
    })?;

    let query_str = args_obj.get("query")
        .or_else(|| args_obj.get("pattern"))
        .and_then(|q| q.as_str())
        .ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required argument 'query' (or 'pattern')".to_string(),
            data: None,
        })?;

    let max_results = args_obj.get("max_results")
        .and_then(|m| m.as_u64())
        .unwrap_or(50)
        .min(200) as usize;

    let regex = regex::Regex::new(query_str).map_err(|e| JsonRpcError {
        code: -32602,
        message: format!("Invalid regular expression: {}", e),
        data: None,
    })?;

    let cwd = env::current_dir().map_err(|e| JsonRpcError {
        code: -32603,
        message: format!("Failed to get current directory: {}", e),
        data: None,
    })?;

    let sub_path = args_obj.get("path").and_then(|p| p.as_str()).unwrap_or(".");
    let target = cwd.join(sub_path);

    let validated = crate::security::validate_path(&target).map_err(|e| JsonRpcError {
        code: -32602,
        message: format!("Security validation failed: {}", e),
        data: None,
    })?;

    let mut matches = Vec::new();

    if validated.is_file() {
        search_file(&validated, &cwd, &regex, &mut matches, max_results);
    } else {
        let walker = build_ignore_walker(&validated, None);
        for entry in walker.build().flatten() {
            if entry.file_type().is_some_and(|ft| ft.is_file()) && crate::security::validate_path(entry.path()).is_ok() {
                search_file(entry.path(), &cwd, &regex, &mut matches, max_results);
                if matches.len() >= max_results {
                    break;
                }
            }
        }
    }

    let out_json = json!({
        "query": query_str,
        "matches_count": matches.len(),
        "matches": matches
    });

    let redacted = crate::security::redact_secrets(&out_json.to_string());

    Ok(CallToolResult {
        content: vec![Content::Text { text: redacted }],
        is_error: None,
    })
}

fn search_file(
    file_path: &std::path::Path,
    root: &std::path::Path,
    regex: &regex::Regex,
    matches: &mut Vec<Value>,
    max_results: usize,
) {
    if matches.len() >= max_results {
        return;
    }
    if let Ok(content) = std::fs::read_to_string(file_path) {
        let rel_path = file_path.strip_prefix(root).unwrap_or(file_path).to_string_lossy().to_string();
        for (idx, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                matches.push(json!({
                    "file": rel_path,
                    "line": idx + 1,
                    "content": line.trim()
                }));
                if matches.len() >= max_results {
                    return;
                }
            }
        }
    }
}

pub fn read_file_tool_def() -> Tool {
    Tool {
        name: "read_file".to_string(),
        description: Some("Reads full or partial content of a file within the workspace (text files, config files, Markdown, source code, etc.).".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative file path within workspace"
                },
                "file_path": {
                    "type": "string",
                    "description": "Alias for path"
                },
                "start_line": {
                    "type": "integer",
                    "description": "Optional 1-based start line number (inclusive)"
                },
                "end_line": {
                    "type": "integer",
                    "description": "Optional 1-based end line number (inclusive)"
                }
            },
            "required": ["path"]
        }),
    }
}

pub fn execute_read_file(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    let args_obj = args.ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing arguments".to_string(),
        data: None,
    })?;

    let path_str = args_obj.get("path")
        .or_else(|| args_obj.get("file_path"))
        .and_then(|p| p.as_str())
        .ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required argument 'path' (or 'file_path')".to_string(),
            data: None,
        })?;

    let validated = crate::security::validate_path(path_str).map_err(|e| JsonRpcError {
        code: -32602,
        message: format!("Security error: {}", e),
        data: None,
    })?;

    if !validated.exists() {
        return Err(JsonRpcError {
            code: -32602,
            message: format!("File not found: {}", path_str),
            data: None,
        });
    }

    if !validated.is_file() {
        return Err(JsonRpcError {
            code: -32602,
            message: format!("Target is not a file: {}", path_str),
            data: None,
        });
    }

    let file_content = fs::read_to_string(&validated).map_err(|e| JsonRpcError {
        code: -32603,
        message: format!("Failed to read file: {}", e),
        data: None,
    })?;

    let lines: Vec<&str> = file_content.lines().collect();
    let total_lines = lines.len();

    let start_line = args_obj.get("start_line").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(1);
    let end_line = args_obj.get("end_line").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(total_lines);

    let effective_start = if start_line == 0 { 1 } else { start_line.min(total_lines.max(1)) };
    let effective_end = end_line.min(total_lines).max(effective_start.saturating_sub(1));

    let sliced_content = if total_lines == 0 || effective_start > total_lines {
        String::new()
    } else {
        lines[effective_start - 1..effective_end].join("\n")
    };

    let redacted_content = crate::security::redact_secrets(&sliced_content);

    let result_json = json!({
        "path": path_str,
        "total_lines": total_lines,
        "start_line": effective_start,
        "end_line": effective_end,
        "content": redacted_content
    });

    Ok(CallToolResult {
        content: vec![Content::Text {
            text: result_json.to_string(),
        }],
        is_error: None,
    })
}

pub fn write_file_tool_def() -> Tool {
    Tool {
        name: "write_file".to_string(),
        description: Some("Writes or updates a file atomically at the specified path within workspace. Supports appending.".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative file path within workspace"
                },
                "content": {
                    "type": "string",
                    "description": "Full file content to write or append"
                },
                "append": {
                    "type": "boolean",
                    "description": "If true, appends content to the end of the file instead of overwriting. Defaults to false."
                }
            },
            "required": ["path", "content"]
        }),
    }
}

pub fn append_file_tool_def() -> Tool {
    Tool {
        name: "append_file".to_string(),
        description: Some("Appends content to the end of a specified file within workspace. Recommended for chunked writes or adding definitions/tests to large files (>3KB) to prevent token truncation.".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative file path within workspace"
                },
                "content": {
                    "type": "string",
                    "description": "Content chunk to append to the end of the file"
                }
            },
            "required": ["path", "content"]
        }),
    }
}

pub fn execute_append_file(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    let mut args_obj = args.ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing arguments".to_string(),
        data: None,
    })?;

    // Auto-decode if arguments arrived as a JSON string from LLM/Hub
    if let Value::String(ref s) = args_obj {
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            args_obj = parsed;
        }
    }

    if let Value::Object(ref mut map) = args_obj {
        map.insert("append".to_string(), Value::Bool(true));
    }

    execute_write_file(Some(args_obj))
}

pub fn execute_write_file(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    let mut args_obj = args.ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing arguments".to_string(),
        data: None,
    })?;

    // Auto-decode if arguments arrived as a JSON string from LLM/Hub
    if let Value::String(ref s) = args_obj {
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            args_obj = parsed;
        }
    }

    let path_str = args_obj.get("path").and_then(|p| p.as_str()).ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing required argument 'path'. Tip: If your tool call was truncated by token limits, split file into smaller chunks using 'append: true' or use 'replace_symbol'.".to_string(),
        data: None,
    })?;

    let content = args_obj.get("content").and_then(|c| c.as_str()).ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing required argument 'content'".to_string(),
        data: None,
    })?;

    let append = args_obj.get("append").and_then(|a| a.as_bool()).unwrap_or(false);

    let validated = crate::security::validate_path(path_str).map_err(|e| JsonRpcError {
        code: -32602,
        message: format!("Security validation failed: {}", e),
        data: None,
    })?;

    if let Some(parent) = validated.parent() {
        std::fs::create_dir_all(parent).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to create parent directories: {}", e),
            data: None,
        })?;
    }

    if append {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&validated)
            .map_err(|e| JsonRpcError {
                code: -32603,
                message: format!("Failed to open file for append: {}", e),
                data: None,
            })?;

        file.write_all(content.as_bytes()).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to append to file: {}", e),
            data: None,
        })?;
    } else {
        std::fs::write(&validated, content).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to write file: {}", e),
            data: None,
        })?;
    }

    Ok(CallToolResult {
        content: vec![Content::Text {
            text: json!({
                "file": path_str,
                "status": "written",
                "bytes": content.len()
            }).to_string(),
        }],
        is_error: None,
    })
}

pub fn run_verification_tool_def() -> Tool {
    let summary = crate::policy::get_policy_summary();
    Tool {
        name: "run_verification".to_string(),
        description: Some(format!(
            "Executes test and build verification commands evaluated by security policy. Policy: [{}]",
            summary
        )),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Verification command or safe '&&' chain evaluated by security policy engine"
                }
            },
            "required": ["command"]
        }),
    }
}

pub fn execute_run_verification(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    let args_obj = args.ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing arguments".to_string(),
        data: None,
    })?;

    let cmd_str = args_obj.get("command")
        .or_else(|| args_obj.get("cmd"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required argument 'command' (or 'cmd')".to_string(),
            data: None,
        })?;

    let (assigned_weight, sub_commands) = match crate::policy::evaluate_command(cmd_str) {
        crate::policy::PolicyCheckResult::Allowed { weight, sub_commands } => (weight, sub_commands),
        crate::policy::PolicyCheckResult::Denied { weight, reason } => {
            return Err(JsonRpcError {
                code: -32602,
                message: format!("Security policy rejected: {}", reason),
                data: Some(json!({
                    "command_weight": weight,
                    "current_mode": crate::policy::get_current_mode().as_str(),
                    "max_allowed_weight": crate::policy::get_max_weight(),
                })),
            });
        }
    };

    let mut all_stdout = String::new();
    let mut all_stderr = String::new();
    let mut overall_success = true;
    let mut last_exit_code = 0;

    for sub_cmd in &sub_commands {
        let tokens: Vec<&str> = sub_cmd.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }

        let program = tokens[0];
        let mut cmd_args: Vec<&str> = tokens[1..].to_vec();
        if program == "go" && tokens.get(1) == Some(&"test") && !tokens.iter().any(|t| t.starts_with("-timeout")) {
            cmd_args.insert(1, "-timeout=30s");
        }

        let output = std::process::Command::new(program)
            .args(&cmd_args)
            .output()
            .map_err(|e| JsonRpcError {
                code: -32603,
                message: format!("Failed to execute sub-command '{}': {}", sub_cmd, e),
                data: None,
            })?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);

        if !all_stdout.is_empty() && !stdout_str.is_empty() {
            all_stdout.push('\n');
        }
        all_stdout.push_str(&stdout_str);

        if !all_stderr.is_empty() && !stderr_str.is_empty() {
            all_stderr.push('\n');
        }
        all_stderr.push_str(&stderr_str);

        last_exit_code = exit_code;
        if !output.status.success() {
            overall_success = false;
            break; // Stop sequential chain on first failure
        }
    }

    let result_json = json!({
        "command": cmd_str,
        "weight": assigned_weight,
        "exit_code": last_exit_code,
        "success": overall_success,
        "stdout": crate::security::redact_secrets(&all_stdout),
        "stderr": crate::security::redact_secrets(&all_stderr),
    });

    Ok(CallToolResult {
        content: vec![Content::Text {
            text: result_json.to_string(),
        }],
        is_error: if overall_success { None } else { Some(true) },
    })
}

// --- Diagnostic Tools ---

pub fn ping_tool_def() -> Tool {
    Tool {
        name: "ping".to_string(),
        description: Some("Returns pong and the current timestamp".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub fn execute_ping() -> Result<CallToolResult, JsonRpcError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    Ok(CallToolResult {
        content: vec![Content::Text {
            text: format!("pong: {}", now),
        }],
        is_error: None,
    })
}

pub fn echo_tool_def() -> Tool {
    Tool {
        name: "echo".to_string(),
        description: Some("Echoes the provided message".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string"
                }
            },
            "required": ["message"]
        }),
    }
}

pub fn execute_echo(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    if let Some(args_obj) = args {
        if let Some(msg) = args_obj.get("message").and_then(|m| m.as_str()) {
            return Ok(CallToolResult {
                content: vec![Content::Text {
                    text: msg.to_string(),
                }],
                is_error: None,
            });
        }
    }

    Err(JsonRpcError {
        code: -32602,
        message: "Missing required argument 'message'".to_string(),
        data: None,
    })
}

pub fn get_status_tool_def() -> Tool {
    Tool {
        name: "get_status".to_string(),
        description: Some("Returns server status, current directory, and version".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub fn execute_get_status() -> Result<CallToolResult, JsonRpcError> {
    let version = env!("CARGO_PKG_VERSION");
    let current_dir = env::current_dir()
        .map(|d| d.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let status_info = json!({
        "status": "running",
        "version": version,
        "current_directory": current_dir,
        "policy": {
            "mode": crate::policy::get_current_mode().as_str(),
            "max_weight": crate::policy::get_max_weight(),
            "summary": crate::policy::get_policy_summary(),
        }
    });

    Ok(CallToolResult {
        content: vec![Content::Text {
            text: status_info.to_string(),
        }],
        is_error: None,
    })
}

// --- Cloud Intelligence Tools (Real WebSocket Calls to Dalembert Hub) ---

pub fn survey_architecture_tool_def() -> Tool {
    Tool {
        name: "survey_architecture".to_string(),
        description: Some("Discovers open-source repo blueprints via Dalembert Cloud Hub".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "repo_full_name": { "type": "string" }
            },
            "required": ["query"]
        }),
    }
}

pub async fn execute_survey_architecture(args: Option<Value>, bridge: &CloudBridge) -> Result<CallToolResult, JsonRpcError> {
    let result = bridge.call_remote_tool("survey_architecture", args.unwrap_or(json!({}))).await?;
    let redacted = crate::security::redact_secrets(&result);

    Ok(CallToolResult {
        content: vec![Content::Text { text: redacted }],
        is_error: None,
    })
}

pub fn search_code_examples_tool_def() -> Tool {
    Tool {
        name: "search_code_examples".to_string(),
        description: Some("Retrieves real-world production code snippets via Dalembert Cloud Hub".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "repo": { "type": "string" },
                "language": { "type": "string" }
            },
            "required": ["query"]
        }),
    }
}

pub async fn execute_search_code_examples(args: Option<Value>, bridge: &CloudBridge) -> Result<CallToolResult, JsonRpcError> {
    let result = bridge.call_remote_tool("search_code_examples", args.unwrap_or(json!({}))).await?;
    let redacted = crate::security::redact_secrets(&result);

    Ok(CallToolResult {
        content: vec![Content::Text { text: redacted }],
        is_error: None,
    })
}

pub fn fetch_doc_tool_def() -> Tool {
    Tool {
        name: "fetch_doc".to_string(),
        description: Some("Pulls sanitized Markdown documentation via Dalembert Cloud Hub".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "url": { "type": "string" }
            },
            "required": ["url"]
        }),
    }
}

pub async fn execute_fetch_doc(args: Option<Value>, bridge: &CloudBridge) -> Result<CallToolResult, JsonRpcError> {
    let result = bridge.call_remote_tool("fetch_doc", args.unwrap_or(json!({}))).await?;
    let redacted = crate::security::redact_secrets(&result);

    Ok(CallToolResult {
        content: vec![Content::Text { text: redacted }],
        is_error: None,
    })
}

pub fn inspect_spec_tool_def() -> Tool {
    Tool {
        name: "inspect_spec".to_string(),
        description: Some("Retrieves test specs as executable living documentation via Dalembert Cloud Hub".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "spec_url": { "type": "string" }
            },
            "required": ["spec_url"]
        }),
    }
}

pub async fn execute_inspect_spec(args: Option<Value>, bridge: &CloudBridge) -> Result<CallToolResult, JsonRpcError> {
    let result = bridge.call_remote_tool("inspect_spec", args.unwrap_or(json!({}))).await?;
    let redacted = crate::security::redact_secrets(&result);

    Ok(CallToolResult {
        content: vec![Content::Text { text: redacted }],
        is_error: None,
    })
}

// --- Dispatchers and Schemas for Cloud / Headless Integration ---

pub async fn execute_local_tool_by_name(name: &str, mut args: Value) -> String {
    // If args arrived as a serialized JSON string, parse it into an Object/Value
    if let Value::String(ref s) = args {
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            args = parsed;
        }
    }
    let args_opt = Some(args);
    let res = match name {
        "get_outline" => execute_get_outline(args_opt),
        "read_symbol" => execute_read_symbol(args_opt),
        "replace_symbol" => execute_replace_symbol(args_opt),
        "read_file" => execute_read_file(args_opt),
        "list_files" => execute_list_files(args_opt),
        "grep_search" => execute_grep_search(args_opt),
        "write_file" => execute_write_file(args_opt),
        "append_file" => execute_append_file(args_opt),
        "run_verification" => execute_run_verification(args_opt),
        "ping" => execute_ping(),
        "echo" => execute_echo(args_opt),
        "get_status" => execute_get_status(),
        "manage_imports" => execute_manage_imports(args_opt),
        _ => Err(JsonRpcError {
            code: -32601,
            message: format!("Unknown local tool: {}", name),
            data: None,
        }),
    };

    match res {
        Ok(call_result) => {
            if let Some(Content::Text { text }) = call_result.content.into_iter().next() {
                text
            } else {
                "ok".to_string()
            }
        }
        Err(e) => format!("Error: {}", e.message),
    }
}

pub fn all_local_tool_schemas() -> Vec<Value> {
    vec![
        get_outline_tool_def(),
        read_symbol_tool_def(),
        replace_symbol_tool_def(),
        read_file_tool_def(),
        list_files_tool_def(),
        grep_search_tool_def(),
        write_file_tool_def(),
        append_file_tool_def(),
        run_verification_tool_def(),
        ping_tool_def(),
        echo_tool_def(),
        get_status_tool_def(),
        manage_imports_tool_def(),
    ]
    .into_iter()
    .map(|t| {
        json!({
            "name": t.name,
            "description": t.description,
            "parameters": t.input_schema,
            "inputSchema": t.input_schema,
        })
    })
    .collect()
}

pub fn all_mcp_tools() -> Vec<Tool> {
    vec![
        get_outline_tool_def(),
        read_symbol_tool_def(),
        replace_symbol_tool_def(),
        read_file_tool_def(),
        list_files_tool_def(),
        grep_search_tool_def(),
        write_file_tool_def(),
        append_file_tool_def(),
        run_verification_tool_def(),
        ping_tool_def(),
        echo_tool_def(),
        get_status_tool_def(),
        manage_imports_tool_def(),
        survey_architecture_tool_def(),
        search_code_examples_tool_def(),
        fetch_doc_tool_def(),
        inspect_spec_tool_def(),
    ]
}

pub fn tool_by_name(name: &str) -> Option<Tool> {
    match name {
        "get_outline" => Some(get_outline_tool_def()),
        "read_symbol" => Some(read_symbol_tool_def()),
        "replace_symbol" => Some(replace_symbol_tool_def()),
        "read_file" => Some(read_file_tool_def()),
        "list_files" => Some(list_files_tool_def()),
        "grep_search" => Some(grep_search_tool_def()),
        "write_file" => Some(write_file_tool_def()),
        "append_file" => Some(append_file_tool_def()),
        "run_verification" => Some(run_verification_tool_def()),
        "ping" => Some(ping_tool_def()),
        "echo" => Some(echo_tool_def()),
        "get_status" => Some(get_status_tool_def()),
        "survey_architecture" => Some(survey_architecture_tool_def()),
        "search_code_examples" => Some(search_code_examples_tool_def()),
        "fetch_doc" => Some(fetch_doc_tool_def()),
        "inspect_spec" => Some(inspect_spec_tool_def()),
        "manage_imports" => Some(manage_imports_tool_def()),
        _ => None,
    }
}

/// Computes the filtered list of MCP tools based on preset, include list, and exclude list.
///
/// Presets:
/// - "full" | "all" (default): All 17 tools
/// - "light" | "plugin": 9 core tools (AST + Cloud Intelligence + Telemetry/Verification)
/// - "ast" | "ast-only": 3 AST structural manipulation tools only
pub fn get_filtered_mcp_tools(
    preset: Option<&str>,
    include: Option<&str>,
    exclude: Option<&str>,
) -> Vec<Tool> {
    let preset_norm = preset
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "full".to_string());

    let mut tools = match preset_norm.as_str() {
        "light" | "plugin" => vec![
            get_outline_tool_def(),
            read_symbol_tool_def(),
            replace_symbol_tool_def(),
            survey_architecture_tool_def(),
            search_code_examples_tool_def(),
            fetch_doc_tool_def(),
            inspect_spec_tool_def(),
            get_status_tool_def(),
            run_verification_tool_def(),
        ],
        "ast" | "ast-only" | "ast_only" => vec![
            get_outline_tool_def(),
            read_symbol_tool_def(),
            replace_symbol_tool_def(),
        ],
        _ => all_mcp_tools(),
    };

    // Apply include (force add tool if not already present)
    if let Some(inc_str) = include {
        for name in inc_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            if !tools.iter().any(|t| t.name == name) {
                if let Some(tool) = tool_by_name(name) {
                    tools.push(tool);
                }
            }
        }
    }

    // Apply exclude (force remove tool if present)
    if let Some(exc_str) = exclude {
        let exclude_set: std::collections::HashSet<&str> = exc_str
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        tools.retain(|t| !exclude_set.contains(t.name.as_str()));
    }

    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo_success() {
        let args = json!({"message": "hello world"});
        let result = execute_echo(Some(args)).unwrap();
        assert_eq!(result.content.len(), 1);
        let Content::Text { text } = &result.content[0];
        assert_eq!(text, "hello world");
    }

    #[test]
    fn test_echo_missing_args() {
        let result = execute_echo(None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[test]
    fn test_ping_execution() {
        let result = execute_ping().unwrap();
        let Content::Text { text } = &result.content[0];
        assert!(text.starts_with("pong: "));
    }

    #[test]
    fn test_get_status_execution() {
        let result = execute_get_status().unwrap();
        let Content::Text { text } = &result.content[0];
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed.get("status").unwrap().as_str().unwrap(), "running");
        assert!(parsed.get("version").is_some());
        assert!(parsed.get("current_directory").is_some());
    }

    #[test]
    fn test_list_files_execution() {
        let result = execute_list_files(None).unwrap();
        let Content::Text { text } = &result.content[0];
        let parsed: Value = serde_json::from_str(text).unwrap();
        let files = parsed.get("files").unwrap().as_array().unwrap();
        assert!(!files.is_empty());
        let has_cargo = files.iter().any(|f| f.as_str().unwrap() == "Cargo.toml");
        assert!(has_cargo);
    }

    #[test]
    fn test_write_and_grep_file() {
        let _temp_dir = std::env::temp_dir();
        let test_file = "test_run_file.txt";
        let target_path = format!("target/{}", test_file);

        // write
        let write_args = json!({
            "path": target_path,
            "content": "Line 1: EnginEdge is ultra fast\nLine 2: Tree-sitter AST engine\nLine 3: Secret token ghp_111111111111111111111111111111111111"
        });
        let write_res = execute_write_file(Some(write_args)).unwrap();
        let Content::Text { text: w_text } = &write_res.content[0];
        assert!(w_text.contains("written"));

        // grep
        let grep_args = json!({
            "query": "Tree-sitter",
            "path": target_path
        });
        let grep_res = execute_grep_search(Some(grep_args)).unwrap();
        let Content::Text { text: g_text } = &grep_res.content[0];
        assert!(g_text.contains("Tree-sitter AST engine"));

        // append
        let append_args = json!({
            "path": target_path,
            "content": "\nLine 4: Appended chunk successfully",
            "append": true
        });
        let append_res = execute_write_file(Some(append_args)).unwrap();
        let Content::Text { text: app_text } = &append_res.content[0];
        assert!(app_text.contains("written"));

        let read_args = json!({
            "path": target_path
        });
        let read_res = execute_read_file(Some(read_args)).unwrap();
        let Content::Text { text: r_text } = &read_res.content[0];
        assert!(r_text.contains("Line 4: Appended chunk successfully"));

        // cleanup
        let _ = std::fs::remove_file(target_path);
    }

    #[test]
    fn test_append_file_tool_execution() {
        let target_path = "target/test_append_tool_file.txt";

        // Initial write using write_file
        let write_args = json!({
            "path": target_path,
            "content": "Line 1: Primary component\n"
        });
        let write_res = execute_write_file(Some(write_args)).unwrap();
        assert_eq!(write_res.is_error, None);

        // Append using append_file
        let append_args = json!({
            "path": target_path,
            "content": "Line 2: Secondary test suite\n"
        });
        let append_res = execute_append_file(Some(append_args)).unwrap();
        assert_eq!(append_res.is_error, None);

        // Read and verify
        let read_args = json!({
            "path": target_path
        });
        let read_res = execute_read_file(Some(read_args)).unwrap();
        let Content::Text { text } = &read_res.content[0];
        assert!(text.contains("Line 1: Primary component"));
        assert!(text.contains("Line 2: Secondary test suite"));

        // Cleanup
        let _ = std::fs::remove_file(target_path);
    }

    #[test]
    fn test_read_file_execution() {
        let args = json!({
            "path": "Cargo.toml",
            "start_line": 1,
            "end_line": 5
        });
        let res = execute_read_file(Some(args)).unwrap();
        assert_eq!(res.is_error, None);
        let Content::Text { text } = &res.content[0];
        assert!(text.contains("Cargo.toml"));
        assert!(text.contains("total_lines"));
        assert!(text.contains("[package]"));
    }

    #[test]
    fn test_run_verification_whitelist() {
        // Dev mode: cargo check (weight 15) is allowed
        crate::policy::init_policy("dev", None);
        let res = execute_run_verification(Some(json!({
            "command": "cargo check"
        }))).unwrap();
        assert_eq!(res.is_error, None);

        // Dev mode: cmake (weight 60) is denied
        let cmake_dev_res = execute_run_verification(Some(json!({
            "command": "cmake -B build"
        })));
        assert!(cmake_dev_res.is_err());

        // Eval mode: cmake (weight 60 <= 75) is allowed to be evaluated
        crate::policy::init_policy("eval", None);

        // Disallowed command (rm -rf / is weight 100)
        let bad_res = execute_run_verification(Some(json!({
            "command": "rm -rf /"
        })));
        assert!(bad_res.is_err());

        // Disallowed chaining with dangerous commands
        let chain_res = execute_run_verification(Some(json!({
            "command": "cargo check && rm -rf /"
        })));
        assert!(chain_res.is_err());

        // Disallowed shell metacharacters
        let pipe_res = execute_run_verification(Some(json!({
            "command": "cargo check | grep error"
        })));
        assert!(pipe_res.is_err());
    }

    #[tokio::test]
    async fn test_cloud_tools_offline_fallback() {
        let bridge = CloudBridge::new(None);

        let survey_res = execute_survey_architecture(Some(json!({"query": "test"})), &bridge).await;
        assert!(survey_res.is_err());
        assert_eq!(survey_res.unwrap_err().code, -32000);

        let search_res = execute_search_code_examples(Some(json!({"query": "test", "lang": "rust"})), &bridge).await;
        assert!(search_res.is_err());
        assert_eq!(search_res.unwrap_err().code, -32000);

        let fetch_res = execute_fetch_doc(Some(json!({"url": "test"})), &bridge).await;
        assert!(fetch_res.is_err());
        assert_eq!(fetch_res.unwrap_err().code, -32000);

        let inspect_res = execute_inspect_spec(Some(json!({"spec_url": "test"})), &bridge).await;
        assert!(inspect_res.is_err());
        assert_eq!(inspect_res.unwrap_err().code, -32000);
    }

    #[test]
    fn test_tools_preset_full() {
        let tools = get_filtered_mcp_tools(Some("full"), None, None);
        assert_eq!(tools.len(), 17);

        let default_tools = get_filtered_mcp_tools(None, None, None);
        assert_eq!(default_tools.len(), 17);
    }

    #[test]
    fn test_tools_preset_light() {
        let tools = get_filtered_mcp_tools(Some("light"), None, None);
        assert_eq!(tools.len(), 9);

        // Verify duplicates and overlaps are stripped
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(!names.contains(&"read_file"));
        assert!(!names.contains(&"write_file"));
        assert!(!names.contains(&"append_file"));
        assert!(!names.contains(&"list_files"));
        assert!(!names.contains(&"grep_search"));
        assert!(!names.contains(&"ping"));
        assert!(!names.contains(&"echo"));

        // Verify core differentiators are present
        assert!(names.contains(&"get_outline"));
        assert!(names.contains(&"read_symbol"));
        assert!(names.contains(&"replace_symbol"));
        assert!(names.contains(&"survey_architecture"));
        assert!(names.contains(&"get_status"));
        assert!(names.contains(&"run_verification"));
    }

    #[test]
    fn test_tools_preset_ast_only() {
        let tools = get_filtered_mcp_tools(Some("ast-only"), None, None);
        assert_eq!(tools.len(), 3);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["get_outline", "read_symbol", "replace_symbol"]);
    }

    #[test]
    fn test_tools_preset_light_with_include_and_exclude() {
        // Include read_file into light preset
        let tools_with_inc = get_filtered_mcp_tools(Some("light"), Some("read_file"), None);
        assert_eq!(tools_with_inc.len(), 10);
        let names_inc: Vec<&str> = tools_with_inc.iter().map(|t| t.name.as_str()).collect();
        assert!(names_inc.contains(&"read_file"));

        // Exclude run_verification from light preset
        let tools_with_exc = get_filtered_mcp_tools(Some("light"), None, Some("run_verification"));
        assert_eq!(tools_with_exc.len(), 8);
        let names_exc: Vec<&str> = tools_with_exc.iter().map(|t| t.name.as_str()).collect();
        assert!(!names_exc.contains(&"run_verification"));

        // Combined include & exclude
        let tools_combined = get_filtered_mcp_tools(Some("light"), Some("ping,echo"), Some("survey_architecture"));
        let names_comb: Vec<&str> = tools_combined.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names_comb.len(), 10); // 9 + 2 - 1 = 10
        assert!(names_comb.contains(&"ping"));
        assert!(names_comb.contains(&"echo"));
        assert!(!names_comb.contains(&"survey_architecture"));
    }
}
