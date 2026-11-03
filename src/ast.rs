use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;
use serde_json::{json, Value};
use tree_sitter::{Node, Parser};

pub fn get_language(extension: &str) -> Result<tree_sitter::Language> {
    match extension {
        "rs" => Ok(tree_sitter_rust::LANGUAGE.into()),
        "go" => Ok(tree_sitter_go::LANGUAGE.into()),
        "ts" | "tsx" => Ok(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "js" | "jsx" => Ok(tree_sitter_javascript::LANGUAGE.into()),
        "py" => Ok(tree_sitter_python::LANGUAGE.into()),
        "rb" => Ok(tree_sitter_ruby::LANGUAGE.into()),
        _ => Err(anyhow!("Unsupported file extension: {}", extension)),
    }
}

pub fn get_language_for_file(file_path: &str) -> Result<tree_sitter::Language> {
    let path = Path::new(file_path);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| anyhow!("No file extension found"))?;
    get_language(ext)
}

fn is_outline_node(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "impl_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "function_declaration"
            | "method_declaration"
            | "class_declaration"
            | "interface_declaration"
            | "function_definition"
            | "class_definition"
            | "method_definition"
            | "method"
            | "class"
            | "function"
    )
}

fn extract_name<'a>(node: Node<'a>, source_code: &'a [u8]) -> Option<String> {
    for i in 0..node.child_count() {
        let child = node.child(i)?;
        if child.kind() == "identifier" || child.kind() == "type_identifier" || child.kind() == "name" {
            if let Ok(name) = child.utf8_text(source_code) {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn find_symbol_node<'a>(
    node: Node<'a>,
    source_code: &'a [u8],
    symbol_path: &str,
) -> Option<Node<'a>> {
    let mut nodes_to_visit = vec![node];

    // Extract tail name if path contains ::, #, or . (e.g. CloudBridge::new or Session#handle_init)
    let tail_name = symbol_path
        .split("::")
        .last()
        .and_then(|s| s.split('#').next_back())
        .and_then(|s| s.split('.').next_back())
        .unwrap_or(symbol_path);

    let mut fallback_node = None;

    while let Some(current) = nodes_to_visit.pop() {
        if is_outline_node(current.kind()) {
            if let Some(name) = extract_name(current, source_code) {
                if name == symbol_path {
                    return Some(current);
                }
                if name == tail_name && fallback_node.is_none() {
                    fallback_node = Some(current);
                }
            }
        }
        for i in (0..current.child_count()).rev() {
            if let Some(child) = current.child(i) {
                nodes_to_visit.push(child);
            }
        }
    }
    fallback_node
}

pub fn read_symbol(file_path: &str, symbol_path: &str) -> Result<Value> {
    let validated_path = crate::security::validate_path(file_path)?;
    let file_path = validated_path.to_str().ok_or_else(|| anyhow!("Invalid UTF-8 in path"))?;
    let language = get_language_for_file(file_path)?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let source_code = fs::read(file_path)?;
    let tree = parser
        .parse(&source_code, None)
        .ok_or_else(|| anyhow!("Failed to parse code"))?;

    let node = find_symbol_node(tree.root_node(), &source_code, symbol_path)
        .ok_or_else(|| anyhow!("Symbol {} not found in file", symbol_path))?;

    let code = node.utf8_text(&source_code)?.to_string();

    Ok(json!({
        "file": file_path,
        "symbol": symbol_path,
        "code": code,
    }))
}

pub fn replace_symbol(file_path: &str, symbol_path: &str, new_code: &str) -> Result<Value> {
    let validated_path = crate::security::validate_path(file_path)?;
    let file_path = validated_path.to_str().ok_or_else(|| anyhow!("Invalid UTF-8 in path"))?;
    let language = get_language_for_file(file_path)?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let source_code = fs::read(file_path)?;
    let tree = parser
        .parse(&source_code, None)
        .ok_or_else(|| anyhow!("Failed to parse code"))?;

    let node = find_symbol_node(tree.root_node(), &source_code, symbol_path)
        .ok_or_else(|| anyhow!("Symbol {} not found in file", symbol_path))?;

    let start_byte = node.start_byte();
    let end_byte = node.end_byte();

    let mut new_source = Vec::new();
    new_source.extend_from_slice(&source_code[..start_byte]);
    new_source.extend_from_slice(new_code.as_bytes());
    new_source.extend_from_slice(&source_code[end_byte..]);

    // Syntax validation
    let mut validation_parser = Parser::new();
    validation_parser.set_language(&language)?;
    let new_tree = validation_parser
        .parse(&new_source, None)
        .ok_or_else(|| anyhow!("Failed to parse replaced code"))?;

    if new_tree.root_node().has_error() {
        return Err(anyhow!("Syntax error introduced by replacement"));
    }

    fs::write(file_path, &new_source)?;

    Ok(json!({
        "file": file_path,
        "symbol": symbol_path,
        "status": "replaced",
    }))
}

pub fn get_outline(file_path: &str) -> Result<Value> {
    let validated_path = crate::security::validate_path(file_path)?;
    let file_path = validated_path.to_str().ok_or_else(|| anyhow!("Invalid UTF-8 in path"))?;
    let language = get_language_for_file(file_path)?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let source_code = fs::read(file_path)?;
    let tree = parser
        .parse(&source_code, None)
        .ok_or_else(|| anyhow!("Failed to parse code"))?;

    let mut symbols = Vec::new();

    let mut nodes_to_visit = vec![tree.root_node()];

    while let Some(node) = nodes_to_visit.pop() {
        if is_outline_node(node.kind()) {
            let name = extract_name(node, &source_code).unwrap_or_else(|| "<anonymous>".to_string());
            let start_pos = node.start_position();
            let end_pos = node.end_position();

            symbols.push(json!({
                "kind": node.kind(),
                "name": name,
                "range": {
                    "start": {
                        "row": start_pos.row,
                        "column": start_pos.column,
                    },
                    "end": {
                        "row": end_pos.row,
                        "column": end_pos.column,
                    }
                }
            }));
        }

        // Add children to the stack (in reverse order so they are processed in order)
        for i in (0..node.child_count()).rev() {
            if let Some(child) = node.child(i) {
                nodes_to_visit.push(child);
            }
        }
    }

    Ok(json!({
        "file": file_path,
        "symbols": symbols,
    }))
}

fn is_import_node(kind: &str) -> bool {
    matches!(
        kind,
        "use_declaration"
            | "import_declaration"
            | "import_statement"
            | "import_from_statement"
    )
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportInfo {
    pub kind: String,
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

pub fn list_imports(file_path: &str) -> Result<Vec<ImportInfo>> {
    let validated_path = crate::security::validate_path(file_path)?;
    let file_path = validated_path.to_str().ok_or_else(|| anyhow!("Invalid UTF-8 in path"))?;
    let language = get_language_for_file(file_path)?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let source_code = fs::read(file_path)?;
    let tree = parser
        .parse(&source_code, None)
        .ok_or_else(|| anyhow!("Failed to parse code"))?;

    let mut imports = Vec::new();
    let root = tree.root_node();
    for i in 0..root.child_count() {
        if let Some(child) = root.child(i) {
            if is_import_node(child.kind()) {
                if let Ok(text) = child.utf8_text(&source_code) {
                    imports.push(ImportInfo {
                        kind: child.kind().to_string(),
                        text: text.trim().to_string(),
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        start_byte: child.start_byte(),
                        end_byte: child.end_byte(),
                    });
                }
            }
        }
    }
    Ok(imports)
}

pub fn add_import(file_path: &str, statement: &str) -> Result<Value> {
    let validated_path = crate::security::validate_path(file_path)?;
    let file_path = validated_path.to_str().ok_or_else(|| anyhow!("Invalid UTF-8 in path"))?;
    let language = get_language_for_file(file_path)?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let source_code = fs::read(file_path)?;
    let tree = parser
        .parse(&source_code, None)
        .ok_or_else(|| anyhow!("Failed to parse code"))?;

    let trimmed_statement = statement.trim();
    if trimmed_statement.is_empty() {
        return Err(anyhow!("Import statement cannot be empty"));
    }

    let existing_imports = list_imports(file_path)?;
    for imp in &existing_imports {
        if imp.text.trim() == trimmed_statement {
            return Ok(json!({
                "status": "noop",
                "message": "Import already exists",
                "statement": trimmed_statement,
                "file": file_path
            }));
        }
    }

    let insert_pos;
    let insert_text;

    if let Some(last_import) = existing_imports.last() {
        insert_pos = last_import.end_byte;
        insert_text = format!("\n{}", trimmed_statement);
    } else {
        let root = tree.root_node();
        let mut header_end = 0;
        for i in 0..root.child_count() {
            if let Some(child) = root.child(i) {
                let kind = child.kind();
                if kind == "comment" || kind == "inner_attribute_item" || kind == "shebang" {
                    header_end = child.end_byte();
                } else if kind == "expression_statement" {
                    if let Some(c0) = child.child(0) {
                        if c0.kind() == "string" && header_end == 0 {
                            header_end = child.end_byte();
                        }
                    }
                    break;
                } else {
                    break;
                }
            }
        }

        if header_end > 0 {
            insert_pos = header_end;
            insert_text = format!("\n\n{}\n", trimmed_statement);
        } else {
            insert_pos = 0;
            insert_text = format!("{}\n\n", trimmed_statement);
        }
    }

    let mut new_source = Vec::new();
    new_source.extend_from_slice(&source_code[..insert_pos]);
    new_source.extend_from_slice(insert_text.as_bytes());
    new_source.extend_from_slice(&source_code[insert_pos..]);

    let mut validation_parser = Parser::new();
    validation_parser.set_language(&language)?;
    let new_tree = validation_parser
        .parse(&new_source, None)
        .ok_or_else(|| anyhow!("Failed to parse code with added import"))?;

    if new_tree.root_node().has_error() {
        return Err(anyhow!("Syntax error introduced by import statement: '{}'", trimmed_statement));
    }

    fs::write(file_path, &new_source)?;

    Ok(json!({
        "status": "success",
        "action": "add",
        "statement": trimmed_statement,
        "file": file_path
    }))
}

pub fn prune_imports(file_path: &str, auto_remove: bool) -> Result<Value> {
    let validated_path = crate::security::validate_path(file_path)?;
    let file_path = validated_path.to_str().ok_or_else(|| anyhow!("Invalid UTF-8 in path"))?;
    let language = get_language_for_file(file_path)?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let source_code = fs::read(file_path)?;
    let tree = parser
        .parse(&source_code, None)
        .ok_or_else(|| anyhow!("Failed to parse code"))?;

    let root = tree.root_node();
    let existing_imports = list_imports(file_path)?;
    if existing_imports.is_empty() {
        return Ok(json!({
            "status": "noop",
            "message": "No imports found in file",
            "file": file_path,
            "unused_imports": []
        }));
    }

    let mut used_identifiers = std::collections::HashSet::new();
    let mut nodes_to_visit = vec![root];
    while let Some(current) = nodes_to_visit.pop() {
        if is_import_node(current.kind()) {
            continue;
        }
        if matches!(current.kind(), "identifier" | "type_identifier") {
            if let Ok(name) = current.utf8_text(&source_code) {
                used_identifiers.insert(name.to_string());
            }
        }
        for i in (0..current.child_count()).rev() {
            if let Some(child) = current.child(i) {
                nodes_to_visit.push(child);
            }
        }
    }

    let mut unused_imports = Vec::new();
    let mut ranges_to_remove = Vec::new();

    for imp in &existing_imports {
        let mut symbols = Vec::new();
        let imp_node = find_node_at_range(root, imp.start_byte, imp.end_byte);
        if let Some(node) = imp_node {
            collect_import_symbols(node, &source_code, &mut symbols);
            let is_used = symbols.is_empty() || symbols.iter().any(|sym| used_identifiers.contains(sym));
            if !is_used {
                unused_imports.push(imp.text.clone());
                ranges_to_remove.push((imp.start_byte, imp.end_byte));
            }
        }
    }

    if auto_remove && !ranges_to_remove.is_empty() {
        ranges_to_remove.sort_by_key(|a| std::cmp::Reverse(a.0));
        let mut new_source = source_code.clone();
        for (start, end) in ranges_to_remove {
            let mut end_with_nl = end;
            if end_with_nl < new_source.len() && new_source[end_with_nl] == b'\n' {
                end_with_nl += 1;
            }
            new_source.drain(start..end_with_nl);
        }

        let mut validation_parser = Parser::new();
        validation_parser.set_language(&language)?;
        if let Some(new_tree) = validation_parser.parse(&new_source, None) {
            if !new_tree.root_node().has_error() {
                fs::write(file_path, &new_source)?;
                return Ok(json!({
                    "status": "pruned",
                    "file": file_path,
                    "removed_imports": unused_imports
                }));
            }
        }
    }

    Ok(json!({
        "status": if unused_imports.is_empty() { "clean" } else { "detected" },
        "file": file_path,
        "unused_imports": unused_imports
    }))
}

fn find_node_at_range<'a>(root: Node<'a>, start_byte: usize, end_byte: usize) -> Option<Node<'a>> {
    let mut to_visit = vec![root];
    while let Some(curr) = to_visit.pop() {
        if curr.start_byte() == start_byte && curr.end_byte() == end_byte {
            return Some(curr);
        }
        for i in 0..curr.child_count() {
            if let Some(child) = curr.child(i) {
                if child.start_byte() <= start_byte && child.end_byte() >= end_byte {
                    to_visit.push(child);
                }
            }
        }
    }
    None
}

fn collect_import_symbols(node: Node, source_code: &[u8], symbols: &mut Vec<String>) {
    let mut to_visit = vec![node];
    while let Some(curr) = to_visit.pop() {
        let kind = curr.kind();
        if kind == "wildcard_import" || kind == "use_wildcard" {
            return;
        }
        if kind == "identifier" || kind == "type_identifier" {
            if let Ok(text) = curr.utf8_text(source_code) {
                symbols.push(text.to_string());
            }
        }
        for i in (0..curr.child_count()).rev() {
            if let Some(child) = curr.child(i) {
                to_visit.push(child);
            }
        }
    }
}

pub fn manage_imports(
    file_path: &str,
    action: &str,
    statement: Option<&str>,
    auto_remove: Option<bool>,
) -> Result<Value> {
    match action {
        "list" => {
            let imports = list_imports(file_path)?;
            Ok(json!({
                "file": file_path,
                "total_imports": imports.len(),
                "imports": imports
            }))
        }
        "add" => {
            let stmt = statement.ok_or_else(|| anyhow!("'statement' argument is required for action 'add'"))?;
            add_import(file_path, stmt)
        }
        "prune" => {
            let remove = auto_remove.unwrap_or(false);
            prune_imports(file_path, remove)
        }
        _ => Err(anyhow!("Unsupported action '{}'. Expected 'list', 'add', or 'prune'", action)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_language() {
        assert!(get_language("rs").is_ok());
        assert!(get_language("go").is_ok());
        assert!(get_language("ts").is_ok());
        assert!(get_language("js").is_ok());
        assert!(get_language("py").is_ok());
        assert!(get_language("rb").is_ok());
        assert!(get_language("cpp").is_err());
    }

    #[test]
    fn test_get_outline() {
        // Write a small temp rs file
        let temp_dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let temp_dir = temp_dir.path();
        let file_path = temp_dir.join("test_outline.rs");
        let code = r#"
            struct Point { x: i32, y: i32 }
            fn add(a: i32, b: i32) -> i32 { a + b }
        "#;
        std::fs::write(&file_path, code).unwrap();

        let outline = get_outline(file_path.to_str().unwrap()).unwrap();
        let symbols = outline.get("symbols").unwrap().as_array().unwrap();

        assert_eq!(symbols.len(), 2);

        let s1 = &symbols[0];
        assert_eq!(s1["kind"], "struct_item");
        assert_eq!(s1["name"], "Point");

        let s2 = &symbols[1];
        assert_eq!(s2["kind"], "function_item");
        assert_eq!(s2["name"], "add");

        std::fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_read_symbol() {
        let temp_dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let temp_dir = temp_dir.path();
        let file_path = temp_dir.join("test_read_symbol.rs");
        let code = r#"
            struct Point { x: i32, y: i32 }
            fn add(a: i32, b: i32) -> i32 { a + b }
        "#;
        std::fs::write(&file_path, code).unwrap();

        let result = read_symbol(file_path.to_str().unwrap(), "add").unwrap();
        let read_code = result.get("code").unwrap().as_str().unwrap();

        assert_eq!(read_code, "fn add(a: i32, b: i32) -> i32 { a + b }");

        // Test path tolerance (e.g. MyStruct::add)
        let result_path = read_symbol(file_path.to_str().unwrap(), "MyStruct::add").unwrap();
        assert_eq!(result_path.get("code").unwrap().as_str().unwrap(), "fn add(a: i32, b: i32) -> i32 { a + b }");

        let result_err = read_symbol(file_path.to_str().unwrap(), "non_existent");
        assert!(result_err.is_err());

        std::fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_replace_symbol() {
        let temp_dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let temp_dir = temp_dir.path();
        let file_path = temp_dir.join("test_replace_symbol.rs");
        let code = r#"
            struct Point { x: i32, y: i32 }
            fn add(a: i32, b: i32) -> i32 { a + b }
        "#;
        std::fs::write(&file_path, code).unwrap();

        // valid replacement
        let res = replace_symbol(file_path.to_str().unwrap(), "add", "fn add(a: i32, b: i32) -> i32 { a + b + 1 }");
        assert!(res.is_ok());

        let new_code = std::fs::read_to_string(&file_path).unwrap();
        assert!(new_code.contains("a + b + 1"));

        // invalid replacement should fail
        let res2 = replace_symbol(file_path.to_str().unwrap(), "Point", "struct Point { x: i32, y: i32, ");
        assert!(res2.is_err());
        assert_eq!(res2.unwrap_err().to_string(), "Syntax error introduced by replacement");

        std::fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_manage_imports() {
        let temp_dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let temp_dir = temp_dir.path();
        let file_path = temp_dir.join("test_imports.py");
        let initial_code = r#""""Module docstring."""
import sys

def main():
    print(sys.version)
"#;
        std::fs::write(&file_path, initial_code).unwrap();
        let path_str = file_path.to_str().unwrap();

        // 1. List imports
        let list_res = manage_imports(path_str, "list", None, None).unwrap();
        assert_eq!(list_res["total_imports"], 1);

        // 2. Add new import
        let add_res = manage_imports(path_str, "add", Some("import os"), None).unwrap();
        assert_eq!(add_res["status"], "success");

        let updated_code = std::fs::read_to_string(&file_path).unwrap();
        assert!(updated_code.contains("import os"));

        // 3. Add duplicate import (noop)
        let dup_res = manage_imports(path_str, "add", Some("import os"), None).unwrap();
        assert_eq!(dup_res["status"], "noop");

        // 4. Prune unused imports: 'os' is unused, 'sys' is used
        let prune_res = manage_imports(path_str, "prune", None, Some(false)).unwrap();
        let unused = prune_res["unused_imports"].as_array().unwrap();
        assert!(unused.iter().any(|u| u.as_str().unwrap() == "import os"));

        // 5. Auto-remove unused imports
        let remove_res = manage_imports(path_str, "prune", None, Some(true)).unwrap();
        assert_eq!(remove_res["status"], "pruned");

        let final_code = std::fs::read_to_string(&file_path).unwrap();
        assert!(!final_code.contains("import os"));
        assert!(final_code.contains("import sys"));

        std::fs::remove_file(file_path).unwrap();
    }
}
