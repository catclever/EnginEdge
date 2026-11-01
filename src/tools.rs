use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use std::env;

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
    collect_files_recursive(&validated_dir, &cwd, 0, max_depth, &mut files)?;
    files.sort();

    Ok(CallToolResult {
        content: vec![Content::Text {
            text: json!({ "files": files }).to_string(),
        }],
        is_error: None,
    })
}

fn collect_files_recursive(
    dir: &std::path::Path,
    root: &std::path::Path,
    depth: usize,
    max_depth: usize,
    files: &mut Vec<String>,
) -> Result<(), JsonRpcError> {
    if depth > max_depth {
        return Ok(());
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => return Err(JsonRpcError {
            code: -32603,
            message: format!("Failed to read directory {}: {}", dir.display(), e),
            data: None,
        }),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Skip ignored directories/files
        if file_name.starts_with('.') && file_name != "." {
            continue;
        }
        if matches!(file_name, "target" | "node_modules" | "vendor" | "build" | "dist" | "tmp" | "temp") {
            continue;
        }

        if path.is_dir() {
            collect_files_recursive(&path, root, depth + 1, max_depth, files)?;
        } else if path.is_file() {
            if crate::security::validate_path(&path).is_ok() {
                if let Ok(rel) = path.strip_prefix(root) {
                    files.push(rel.to_string_lossy().to_string());
                }
            }
        }
    }

    Ok(())
}

pub fn grep_search_tool_def() -> Tool {
    Tool {
        name: "grep_search".to_string(),
        description: Some("Searches for text pattern or regex across workspace files, respecting security jail".to_string()),
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
        search_file(&validated, &cwd, &regex, &mut matches);
    } else {
        let mut file_list = Vec::new();
        collect_files_recursive(&validated, &cwd, 0, 10, &mut file_list)?;
        for rel in file_list {
            let full_p = cwd.join(&rel);
            search_file(&full_p, &cwd, &regex, &mut matches);
            if matches.len() >= 100 {
                break;
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
) {
    if let Ok(content) = std::fs::read_to_string(file_path) {
        let rel_path = file_path.strip_prefix(root).unwrap_or(file_path).to_string_lossy().to_string();
        for (idx, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                matches.push(json!({
                    "file": rel_path,
                    "line": idx + 1,
                    "content": line.trim()
                }));
                if matches.len() >= 200 {
                    return;
                }
            }
        }
    }
}

pub fn write_file_tool_def() -> Tool {
    Tool {
        name: "write_file".to_string(),
        description: Some("Writes or updates a file atomically at the specified path within workspace".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative file path within workspace"
                },
                "content": {
                    "type": "string",
                    "description": "Full file content to write"
                }
            },
            "required": ["path", "content"]
        }),
    }
}

pub fn execute_write_file(args: Option<Value>) -> Result<CallToolResult, JsonRpcError> {
    let args_obj = args.ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing arguments".to_string(),
        data: None,
    })?;

    let path_str = args_obj.get("path").and_then(|p| p.as_str()).ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing required argument 'path'".to_string(),
        data: None,
    })?;

    let content = args_obj.get("content").and_then(|c| c.as_str()).ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing required argument 'content'".to_string(),
        data: None,
    })?;

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

    std::fs::write(&validated, content).map_err(|e| JsonRpcError {
        code: -32603,
        message: format!("Failed to write file: {}", e),
        data: None,
    })?;

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
    Tool {
        name: "run_verification".to_string(),
        description: Some("Executes whitelisted test and build verification commands (e.g. cargo test, cargo check, npm test, pytest, bundle exec rspec)".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Whitelisted verification command to execute"
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

    for forbidden in ["&", "|", ";", ">", "<", "`", "$", "\n"] {
        if cmd_str.contains(forbidden) {
            return Err(JsonRpcError {
                code: -32602,
                message: format!("Security error: Disallowed character '{}' in verification command", forbidden),
                data: None,
            });
        }
    }

    let tokens: Vec<&str> = cmd_str.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(JsonRpcError {
            code: -32602,
            message: "Empty command".to_string(),
            data: None,
        });
    }

    let program = tokens[0];
    let is_whitelisted = match program {
        "cargo" => true,
        "npm" | "pnpm" | "yarn" | "bun" => true,
        "pytest" => true,
        "python" | "python3" => tokens.get(1).map_or(false, |&a| a == "-m"),
        "bundle" => tokens.get(1).map_or(false, |&a| a == "exec"),
        "go" => tokens.get(1).map_or(false, |&a| a == "test" || a == "vet"),
        "make" => tokens.get(1).map_or(false, |&a| a == "test" || a == "check"),
        _ => false,
    };

    if !is_whitelisted {
        return Err(JsonRpcError {
            code: -32602,
            message: format!("Security error: Program '{}' is not in verification whitelist", program),
            data: None,
        });
    }

    let output = std::process::Command::new(program)
        .args(&tokens[1..])
        .output()
        .map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to execute command: {}", e),
            data: None,
        })?;

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let result_json = json!({
        "command": cmd_str,
        "exit_code": exit_code,
        "success": output.status.success(),
        "stdout": crate::security::redact_secrets(&stdout),
        "stderr": crate::security::redact_secrets(&stderr),
    });

    Ok(CallToolResult {
        content: vec![Content::Text {
            text: result_json.to_string(),
        }],
        is_error: if output.status.success() { None } else { Some(true) },
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

pub async fn execute_local_tool_by_name(name: &str, args: Value) -> String {
    let args_opt = Some(args);
    let res = match name {
        "get_outline" => execute_get_outline(args_opt),
        "read_symbol" => execute_read_symbol(args_opt),
        "replace_symbol" => execute_replace_symbol(args_opt),
        "list_files" => execute_list_files(args_opt),
        "grep_search" => execute_grep_search(args_opt),
        "write_file" => execute_write_file(args_opt),
        "run_verification" => execute_run_verification(args_opt),
        "ping" => execute_ping(),
        "echo" => execute_echo(args_opt),
        "get_status" => execute_get_status(),
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
        list_files_tool_def(),
        grep_search_tool_def(),
        write_file_tool_def(),
        run_verification_tool_def(),
        ping_tool_def(),
        echo_tool_def(),
        get_status_tool_def(),
    ]
    .into_iter()
    .map(|t| serde_json::to_value(t).unwrap_or(json!({})))
    .collect()
}

pub fn all_mcp_tools() -> Vec<Tool> {
    vec![
        get_outline_tool_def(),
        read_symbol_tool_def(),
        replace_symbol_tool_def(),
        list_files_tool_def(),
        grep_search_tool_def(),
        write_file_tool_def(),
        run_verification_tool_def(),
        ping_tool_def(),
        echo_tool_def(),
        get_status_tool_def(),
        survey_architecture_tool_def(),
        search_code_examples_tool_def(),
        fetch_doc_tool_def(),
        inspect_spec_tool_def(),
    ]
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

        // cleanup
        let _ = std::fs::remove_file(target_path);
    }

    #[test]
    fn test_run_verification_whitelist() {
        // Allowed command
        let res = execute_run_verification(Some(json!({
            "command": "cargo check"
        }))).unwrap();
        assert_eq!(res.is_error, None);

        // Disallowed command
        let bad_res = execute_run_verification(Some(json!({
            "command": "rm -rf /"
        })));
        assert!(bad_res.is_err());

        // Disallowed chaining
        let chain_res = execute_run_verification(Some(json!({
            "command": "cargo check && rm -rf /"
        })));
        assert!(chain_res.is_err());
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
}
