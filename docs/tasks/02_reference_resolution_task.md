# Task: Reference Resolution

## Description
Implement proper symbol reference resolution to link call sites to their definitions. Currently, call references are tracked but not resolved to actual symbol IDs.

## Requirements
- Resolve function calls to their definitions across files
- Build complete call graph with proper references
- Support go-to-definition functionality
- Support find-all-references functionality
- Handle method resolution (struct.field or struct.method)

## Technical Approach
1. Build global symbol index by name
2. Resolve call references using symbol name lookup
3. Update ReferenceIndex to store resolved references
4. Add `resolve_symbol()` method to CodeSift
5. Add `find_references()` method to CodeSift

## Files to Modify
- `src/graph.rs` - Add reference resolution
- `src/parser.rs` - Enhance call extraction
- `src/lib.rs` - Add resolution methods

## Acceptance Criteria
- [x] Calls are linked to their definitions
- [x] `find_references()` returns all call sites
- [x] `get_definition()` returns the definition site
- [x] Cross-file references are resolved

## Status
- [x] Completed
