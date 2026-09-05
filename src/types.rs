//! Core types for CodeSift.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unique identifier for a file in the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileId(pub u64);

impl FileId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Unique identifier for a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId(pub u64);

impl SymbolId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Language identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Language {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Go,
    Java,
    C,
    Cpp,
    Html,
    Css,
    Json,
    Toml,
    Markdown,
    Unknown,
}

impl Language {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Language::Rust,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "ts" | "tsx" | "mts" | "cts" => Language::TypeScript,
            "jsx" => Language::JavaScript,
            "py" | "pyw" => Language::Python,
            "go" => Language::Go,
            "java" => Language::Java,
            "c" => Language::C,
            "h" | "hpp" => Language::Cpp,
            "cpp" | "cc" | "cxx" | "c++" => Language::Cpp,
            "html" | "htm" => Language::Html,
            "css" | "scss" | "sass" | "less" => Language::Css,
            "json" => Language::Json,
            "toml" => Language::Toml,
            "md" | "markdown" => Language::Markdown,
            _ => Language::Unknown,
        }
    }

    pub fn tree_sitter_lang(&self) -> Option<&'static str> {
        match self {
            Language::Rust => Some("rust"),
            Language::JavaScript => Some("javascript"),
            Language::TypeScript => Some("typescript"),
            Language::Python => Some("python"),
            Language::Go => Some("go"),
            Language::Java => Some("java"),
            Language::C => Some("c"),
            Language::Cpp => Some("cpp"),
            Language::Html => Some("html"),
            Language::Css => Some("css"),
            _ => None,
        }
    }
}

/// Source code range (byte offset in file).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Range {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: u32,
    pub end_line: u32,
}

impl Range {
    pub fn new(start_byte: usize, end_byte: usize, start_line: u32, end_line: u32) -> Self {
        Self {
            start_byte,
            end_byte,
            start_line,
            end_line,
        }
    }

    pub fn contains(&self, byte: usize) -> bool {
        self.start_byte <= byte && byte < self.end_byte
    }

    pub fn contains_range(&self, other: &Range) -> bool {
        self.start_byte <= other.start_byte && self.end_byte >= other.end_byte
    }

    pub fn overlaps(&self, other: &Range) -> bool {
        self.start_byte < other.end_byte && other.start_byte < self.end_byte
    }

    pub fn line_count(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }
}

/// Symbol kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum SymbolKind {
    Module,
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Impl,
    Field,
    Constant,
    Variable,
    Type,
    Interface,
    Class,
    Import,
    Call,
}

impl SymbolKind {
    pub fn is_definition(&self) -> bool {
        matches!(
            self,
            Self::Module
                | Self::Function
                | Self::Method
                | Self::Struct
                | Self::Enum
                | Self::Trait
                | Self::Impl
                | Self::Field
                | Self::Constant
                | Self::Variable
                | Self::Type
                | Self::Interface
                | Self::Class
        )
    }

    pub fn is_reference(&self) -> bool {
        matches!(self, Self::Import | Self::Call)
    }
}

/// Visibility modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Internal,
}

/// A symbol (function, struct, etc.) in the codebase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub file_id: FileId,
    pub range: Range,
    pub parent: Option<SymbolId>,
    pub visibility: Visibility,
    pub signature: Option<String>,
}

impl Symbol {
    pub fn new(
        id: SymbolId,
        name: String,
        kind: SymbolKind,
        file_id: FileId,
        range: Range,
    ) -> Self {
        Self {
            id,
            name,
            kind,
            file_id,
            range,
            parent: None,
            visibility: Visibility::Public,
            signature: None,
        }
    }

    pub fn with_parent(mut self, parent: SymbolId) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn with_signature(mut self, sig: String) -> Self {
        self.signature = Some(sig);
        self
    }
}

/// File metadata in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub id: FileId,
    pub path: PathBuf,
    pub language: Language,
    pub size_bytes: u64,
    pub line_count: u32,
    pub modified_at: u64,
    pub is_binary: bool,
    pub is_generated: bool,
    pub is_vendor: bool,
}

impl FileEntry {
    pub fn from_path(path: PathBuf) -> Option<Self> {
        let metadata = std::fs::metadata(&path).ok()?;
        let size_bytes = metadata.len();
        let modified_at = metadata
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();

        let extension = path.extension()?.to_str()?;
        let language = Language::from_extension(extension);

        // Count lines (approximate)
        let content = std::fs::read(&path).ok()?;
        let line_count = content.iter().filter(|&&b| b == b'\n').count() as u32 + 1;

        // Simple binary detection
        let is_binary = content.iter().take(8192).any(|&b| b == 0);

        Some(Self {
            id: FileId(0), // Will be assigned by index
            path,
            language,
            size_bytes,
            line_count,
            modified_at,
            is_binary,
            is_generated: false,
            is_vendor: false,
        })
    }
}

/// Relationship between symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Relationship {
    Defines,
    References,
    Calls,
    CalleeOf,
    Implements,
    ExtendedBy,
    Imports,
    Exports,
    Tests,
    TestFor,
}

impl Relationship {
    pub fn is_directed(&self) -> bool {
        matches!(
            self,
            Self::Calls
                | Self::CalleeOf
                | Self::Implements
                | Self::ExtendedBy
                | Self::Imports
                | Self::Exports
        )
    }
}
