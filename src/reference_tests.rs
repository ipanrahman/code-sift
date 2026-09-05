//! Table-driven tests for reference resolution.

#[cfg(test)]
mod tests {
    use crate::{CodeSift, Relationship};
    use std::fs;
    use tempfile::TempDir;

    struct RepoBuilder {
        #[allow(dead_code)]
        temp: TempDir,
        path: std::path::PathBuf,
    }

    impl RepoBuilder {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();
            let path = temp.path().canonicalize().unwrap().to_path_buf();
            Self { temp, path }
        }

        fn write(&self, name: &str, content: &str) {
            fs::write(self.path.join(name), content).unwrap();
        }

        fn open(self) -> CodeSift {
            CodeSift::open(self.path.clone()).unwrap()
        }
    }

    #[derive(Debug)]
    struct Case {
        label: &'static str,
        setup: Vec<(&'static str, &'static str)>,
        query: &'static str,
        want_references: bool,
        want_definition: bool,
    }

    impl Case {
        fn new(label: &'static str) -> Self {
            Self {
                label,
                setup: Vec::new(),
                query: "",
                want_references: false,
                want_definition: false,
            }
        }

        fn with_files(mut self, files: Vec<(&'static str, &'static str)>) -> Self {
            self.setup = files;
            self
        }

        fn query(mut self, q: &'static str) -> Self {
            self.query = q;
            self
        }

        fn expect_references(mut self, want: bool) -> Self {
            self.want_references = want;
            self
        }

        fn expect_definition(mut self, want: bool) -> Self {
            self.want_definition = want;
            self
        }
    }

    fn run_case(case: Case) {
        // Arrange
        let builder = RepoBuilder::new();
        for (name, content) in &case.setup {
            builder.write(name, content);
        }
        let codesift = builder.open();

        // Act
        let refs = codesift.find_references(case.query);
        let defs = codesift.get_definition(case.query);

        // Assert
        if case.want_references {
            assert!(
                !refs.is_empty(),
                "{}: expected references for '{}', got none",
                case.label,
                case.query
            );
        } else {
            assert!(
                refs.is_empty(),
                "{}: expected no references for '{}', got {}",
                case.label,
                case.query,
                refs.len()
            );
        }

        if case.want_definition {
            assert!(
                !defs.is_empty(),
                "{}: expected definition for '{}', got none",
                case.label,
                case.query
            );
        } else {
            assert!(
                defs.is_empty(),
                "{}: expected no definition for '{}', got {}",
                case.label,
                case.query,
                defs.len()
            );
        }
    }

    #[test]
    fn test_reference_resolution_table() {
        let cases: Vec<Case> = vec![
            Case::new("intra_file_call")
                .with_files(vec![(
                    "main.rs",
                    r#"fn helper() {}
fn main() { helper(); }
"#,
                )])
                .query("helper")
                .expect_definition(true)
                .expect_references(true),
            Case::new("definition_only")
                .with_files(vec![("b.rs", r#"fn unused() {}"#)])
                .query("unused")
                .expect_definition(true)
                .expect_references(false),
            Case::new("no_match")
                .with_files(vec![("c.rs", r#"fn main() {}"#)])
                .query("nonexistent")
                .expect_definition(false)
                .expect_references(false),
            Case::new("cross_file_call")
                .with_files(vec![
                    ("lib.rs", r#"pub fn shared() {}"#),
                    ("main.rs", r#"fn start() { shared(); }"#),
                ])
                .query("shared")
                .expect_definition(true)
                .expect_references(true),
        ];

        for case in cases {
            run_case(case);
        }
    }

    #[test]
    fn test_find_references_caller_callee_relationship() {
        // Arrange
        let builder = RepoBuilder::new();
        builder.write(
            "main.rs",
            r#"fn caller() { callee(); }
fn callee() {}
"#,
        );
        let codesift = builder.open();

        let callers = codesift.get_callers(
            codesift
                .find_symbol("callee")
                .first()
                .map(|s| s.id)
                .unwrap(),
        );

        // Act + Assert
        assert!(
            !callers.is_empty(),
            "expected callee to have callers, got none"
        );
        let caller_symbol = codesift.get_symbol(callers[0]).unwrap();
        assert_eq!(caller_symbol.name, "caller");
    }

    #[test]
    fn test_get_definition_returns_symbol() {
        // Arrange
        let builder = RepoBuilder::new();
        builder.write("main.rs", r#"fn my_func() {}"#);
        let codesift = builder.open();

        // Act
        let defs = codesift.get_definition("my_func");

        // Assert
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "my_func");
        assert_eq!(defs[0].file_id, defs[0].file_id);
    }

    #[test]
    fn test_resolve_references_across_files() {
        // Arrange
        let builder = RepoBuilder::new();
        builder.write("lib.rs", r#"pub fn shared() {}"#);
        builder.write("main.rs", r#"fn start() { shared(); }"#);
        let codesift = builder.open();

        // Act
        let refs = codesift.find_references("shared");

        // Assert
        assert!(!refs.is_empty(), "expected cross-file reference");
        let (_, rel) = refs[0];
        assert_eq!(rel, Relationship::Calls);
    }

    #[test]
    fn test_definition_and_reference_in_same_file() {
        // Arrange
        let builder = RepoBuilder::new();
        builder.write(
            "main.rs",
            r#"fn target() {}
fn source() { target(); }
"#,
        );
        let codesift = builder.open();

        // Act
        let defs = codesift.get_definition("target");
        let refs = codesift.find_references("target");

        // Assert
        assert_eq!(defs.len(), 1, "expected exactly 1 definition");
        assert_eq!(defs[0].name, "target");

        assert_eq!(refs.len(), 1, "expected exactly 1 reference");
        assert_eq!(refs[0].0.name, "source");
        assert_eq!(refs[0].1, Relationship::Calls);
    }
}
