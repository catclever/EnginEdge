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

    while let Some(current) = nodes_to_visit.pop() {
        if is_outline_node(current.kind()) {
            if let Some(name) = extract_name(current, source_code) {
                if name == symbol_path {
                    return Some(current);
                }
            }
        }
        for i in (0..current.child_count()).rev() {
            if let Some(child) = current.child(i) {
                nodes_to_visit.push(child);
            }
        }
    }
    None
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
}
