//! Tree-sitter based parser for extracting symbols and references.

use crate::error::{Error, Result};
use crate::types::{Language, Range, Relationship, Symbol, SymbolId, SymbolKind, Visibility};

/// A call reference found during parsing.
#[derive(Debug, Clone)]
pub struct CallReference {
    pub caller: Option<SymbolId>,
    pub callee_name: String,
    pub range: Range,
}

/// Parsed file with symbols and references.
#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub symbols: Vec<Symbol>,
    pub references: Vec<(SymbolId, String, Relationship)>,
    pub calls: Vec<CallReference>,
}

/// Extract symbols from source code using Tree-sitter.
#[derive(Clone)]
pub struct Parser;

impl Parser {
    pub fn new() -> Self {
        Self
    }

    /// Parse source and extract symbols and references.
    pub fn parse(&mut self, source: &[u8], lang: Language, file_id: crate::types::FileId) -> Result<ParsedFile> {
        let tree = match lang {
            Language::Rust => {
                let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
                self.parse_with_lang(source, lang)
            }
            Language::JavaScript | Language::TypeScript => {
                let lang: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();
                self.parse_with_lang(source, lang)
            }
            Language::Python => {
                let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
                self.parse_with_lang(source, lang)
            }
            _ => return Err(Error::Parse(format!("unsupported language: {:?}", lang))),
        }?;

        let (symbols, references, calls) = self.extract_all(&tree, source, file_id);
        Ok(ParsedFile { symbols, references, calls })
    }

    fn parse_with_lang(&mut self, source: &[u8], lang: tree_sitter::Language) -> Result<tree_sitter::Tree> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang)
            .map_err(|e| Error::Parse(e.to_string()))?;

        parser.parse(source, None)
            .ok_or_else(|| Error::Parse("failed to parse source".into()))
    }

    fn extract_all(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        file_id: crate::types::FileId,
    ) -> (Vec<Symbol>, Vec<(SymbolId, String, Relationship)>, Vec<CallReference>) {
        let mut symbols = Vec::new();
        let mut symbol_map: std::collections::HashMap<String, SymbolId> = std::collections::HashMap::new();
        let mut references = Vec::new();
        let mut calls = Vec::new();
        let mut next_id = 0u64;

        let root = tree.root_node();
        self.walk_node(
            root, source, file_id, None, &mut symbols,
            &mut symbol_map, &mut references, &mut calls, &mut next_id,
        );

        (symbols, references, calls)
    }

    fn walk_node(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        file_id: crate::types::FileId,
        parent: Option<SymbolId>,
        symbols: &mut Vec<Symbol>,
        symbol_map: &mut std::collections::HashMap<String, SymbolId>,
        references: &mut Vec<(SymbolId, String, Relationship)>,
        calls: &mut Vec<CallReference>,
        next_id: &mut u64,
    ) {
        let _node_kind = node.kind();

        // Extract call references
        if let Some(call_info) = self.extract_call(node, source, parent) {
            calls.push(call_info);
        }

        if let Some(symbol) = self.node_to_symbol(node, source, file_id, parent) {
            let id = SymbolId::new(*next_id);
            *next_id += 1;

            let mut symbol = symbol;
            symbol.id = id;
            symbol.parent = parent;

            // Register in symbol map for reference resolution
            symbol_map.insert(symbol.name.clone(), id);

            symbols.push(symbol.clone());

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.walk_node(
                    child, source, file_id, Some(id), symbols,
                    symbol_map, references, calls, next_id,
                );
            }
        } else {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                self.walk_node(
                    child, source, file_id, parent, symbols,
                    symbol_map, references, calls, next_id,
                );
            }
        }
    }

    fn extract_call(&self, node: tree_sitter::Node, source: &[u8], caller: Option<SymbolId>) -> Option<CallReference> {
        let kind = node.kind();

        // Rust: function calls, method calls
        if kind == "call_expression" {
            // Get the function being called
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" || child.kind() == "field_expression" {
                    let name = self.extract_text(&child, source)?;
                    let range = self.node_to_range(&child);
                    return Some(CallReference { caller, callee_name: name, range });
                }
            }
        }

        // JavaScript/TypeScript: call expressions
        if kind == "call_expression" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" || child.kind() == "member_expression" {
                    let name = self.extract_text(&child, source)?;
                    let range = self.node_to_range(&child);
                    return Some(CallReference { caller, callee_name: name, range });
                }
            }
        }

        // Python: call expressions
        if kind == "call" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" || child.kind() == "attribute" {
                    let name = self.extract_text(&child, source)?;
                    let range = self.node_to_range(&child);
                    return Some(CallReference { caller, callee_name: name, range });
                }
            }
        }

        None
    }

    fn node_to_range(&self, node: &tree_sitter::Node) -> Range {
        let start = node.start_byte();
        let end = node.end_byte();
        let start_point = node.start_position();
        let end_point = node.end_position();
        Range::new(
            start,
            end,
            start_point.row as u32 + 1,
            end_point.row as u32 + 1,
        )
    }

    fn node_to_symbol(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        file_id: crate::types::FileId,
        parent: Option<SymbolId>,
    ) -> Option<Symbol> {
        let kind = node.kind();
        let (symbol_kind, name) = match kind {
            // Rust
            "function_item" => (SymbolKind::Function, self.find_name_in_node(node, source, &["identifier", "declarator"])),
            "struct_item" => (SymbolKind::Struct, self.find_name_in_node(node, source, &["type_identifier", "identifier"])),
            "enum_item" => (SymbolKind::Enum, self.find_name_in_node(node, source, &["type_identifier", "identifier"])),
            "trait_item" => (SymbolKind::Trait, self.find_name_in_node(node, source, &["type_identifier", "identifier"])),
            "impl_item" => (SymbolKind::Impl, self.find_name_in_node(node, source, &["type_identifier", "identifier"])),
            "const_item" => (SymbolKind::Constant, self.find_name_in_node(node, source, &["identifier"])),

            // JavaScript/TypeScript
            "function_declaration" => (SymbolKind::Function, self.find_name_in_node(node, source, &["identifier", "function"])),
            "class_declaration" => (SymbolKind::Class, self.find_name_in_node(node, source, &["identifier", "class"])),

            // Python
            "function_definition" => (SymbolKind::Function, self.find_name_in_node(node, source, &["identifier"])),
            "class_definition" => (SymbolKind::Class, self.find_name_in_node(node, source, &["identifier"])),

            _ => return None,
        };

        let name = name?;

        let range = self.node_to_range(&node);

        Some(Symbol {
            id: SymbolId::new(0),
            name,
            kind: symbol_kind,
            file_id,
            range,
            parent,
            visibility: Visibility::Public,
            signature: None,
        })
    }

    fn find_name_in_node(&self, node: tree_sitter::Node, source: &[u8], target_kinds: &[&str]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if target_kinds.contains(&child.kind()) {
                if let Some(name) = self.extract_text(&child, source) {
                    return Some(name);
                }
            }
        }
        None
    }

    fn extract_text(&self, node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        let start = node.start_byte();
        let end = node.end_byte();
        source.get(start..end).and_then(|s| String::from_utf8(s.to_vec()).ok())
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}
