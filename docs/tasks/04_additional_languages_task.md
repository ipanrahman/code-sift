# Task: Additional Language Support

## Description
Extend CodeSift to support more programming languages: Go, Java, C/C++, and TypeScript improvements.

## Requirements
- Add Go parser support using tree-sitter-go
- Add Java parser support using tree-sitter-java
- Add C/C++ parser support using tree-sitter-c and tree-sitter-cpp
- Improve TypeScript/JSX support
- Add language-specific symbol kinds for each language

## Languages to Support

### Go
- Functions: `function_declaration`
- Methods: `method_declaration`
- Types: `type_declaration`, `struct_type`
- Interfaces: `interface_type`

### Java
- Classes: `class_declaration`
- Methods: `method_declaration`
- Interfaces: `interface_declaration`
- Fields: `field_declaration`

### C/C++
- Functions: `function_definition`
- Structs: `struct_specifier`
- Classes: `class_specifier`
- Enums: `enum_specifier`

## Technical Approach
1. Add tree-sitter language crates to Cargo.toml
2. Update `parser.rs` to handle new languages
3. Update `types.rs` Language enum with new variants
4. Add language-specific node kinds in parser
5. Update file extension mapping in types.rs

## Files to Modify
- `Cargo.toml` - Add language dependencies
- `src/types.rs` - Add language variants
- `src/parser.rs` - Add language parsers
- `src/repository.rs` - Update extension mapping

## Acceptance Criteria
- [ ] Go files are parsed and symbols extracted
- [ ] Java files are parsed and symbols extracted
- [ ] C/C++ files are parsed and symbols extracted
- [ ] TypeScript/JSX works correctly
- [ ] Tests pass for all languages

## Status
- [ ] Not started
