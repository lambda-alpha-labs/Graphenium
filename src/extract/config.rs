use tree_sitter::Language;

use crate::model::ExtractionResult;

/// Signature for a language-specific import statement handler.
///
/// Called once per import node found during the structure pass.
/// Implementations extract the imported module/name and push nodes/edges
/// onto `result`.
pub type ImportHandlerFn = for<'tree> fn(
    node: tree_sitter::Node<'tree>,
    source: &[u8],
    file_path: &str,
    file_node_id: &str,
    result: &mut ExtractionResult,
);

/// Data-driven configuration for one language's AST structure.
///
/// All languages share the same generic two-pass walker in `walker.rs`.
/// Language-specific behaviour is captured here and in optional handler
/// function pointers.
pub struct LanguageConfig {
    /// The compiled tree-sitter grammar.
    pub language: Language,
    /// Node kinds that represent class / struct / interface definitions.
    pub class_kinds: &'static [&'static str],
    /// Node kinds that represent function / method definitions.
    pub function_kinds: &'static [&'static str],
    /// Node kinds that represent import / using / #include statements.
    pub import_kinds: &'static [&'static str],
    /// Node kinds that represent call expressions.
    pub call_kinds: &'static [&'static str],
    /// The tree-sitter **field** name that holds the node's identifier.
    /// Most languages: `"name"`.  C/C++ function definitions: `"declarator"`.
    pub name_field: &'static str,
    /// Language-specific import handler; `None` = skip imports.
    pub import_handler: Option<ImportHandlerFn>,
}

/// Return the `LanguageConfig` for a file extension, or `None` when the
/// language is unsupported or its feature flag is disabled at compile time.
pub fn config_for_extension(ext: &str) -> Option<LanguageConfig> {
    match ext {
        #[cfg(feature = "lang-python")]
        "py" | "pyw" => Some(python()),

        #[cfg(feature = "lang-js")]
        "js" | "mjs" | "cjs" | "jsx" => Some(javascript()),

        #[cfg(feature = "lang-ts")]
        "ts" => Some(typescript(false)),
        #[cfg(feature = "lang-ts")]
        "tsx" => Some(typescript(true)),

        #[cfg(feature = "lang-java")]
        "java" => Some(java()),

        #[cfg(feature = "lang-c")]
        "c" | "h" => Some(c()),

        #[cfg(feature = "lang-cpp")]
        "cpp" | "cc" | "cxx" | "hh" | "hpp" | "hxx" => Some(cpp()),

        #[cfg(feature = "lang-csharp")]
        "cs" => Some(csharp()),

        _ => None,
    }
}

// ── Per-language factory functions ────────────────────────────────────────────
//
// Go and Rust are handled by their own custom extractors (go.rs / rust_lang.rs)
// rather than the generic walker, so they have no entry here.

#[cfg(feature = "lang-python")]
fn python() -> LanguageConfig {
    use super::import_handlers::python_import;
    LanguageConfig {
        language: tree_sitter_python::LANGUAGE.into(),
        class_kinds: &["class_definition"],
        function_kinds: &["function_definition"],
        import_kinds: &["import_statement", "import_from_statement"],
        call_kinds: &["call"],
        name_field: "name",
        import_handler: Some(python_import),
    }
}

#[cfg(feature = "lang-js")]
fn javascript() -> LanguageConfig {
    use super::import_handlers::js_import_handler;
    LanguageConfig {
        language: tree_sitter_javascript::LANGUAGE.into(),
        class_kinds: &["class_declaration", "class_expression"],
        function_kinds: &[
            "function_declaration",
            "function_expression",
            "method_definition",
        ],
        import_kinds: &["import_statement", "export_statement", "call_expression"],
        call_kinds: &["call_expression"],
        name_field: "name",
        import_handler: Some(js_import_handler),
    }
}

#[cfg(feature = "lang-ts")]
fn typescript(tsx: bool) -> LanguageConfig {
    use super::import_handlers::js_import_handler; // TS imports use the same syntax
    let language = if tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    LanguageConfig {
        language,
        class_kinds: &["class_declaration", "class_expression"],
        function_kinds: &[
            "function_declaration",
            "function_expression",
            "method_definition",
            "abstract_method_signature",
        ],
        import_kinds: &["import_statement", "export_statement", "call_expression"],
        call_kinds: &["call_expression"],
        name_field: "name",
        import_handler: Some(js_import_handler),
    }
}

#[cfg(feature = "lang-java")]
fn java() -> LanguageConfig {
    use super::import_handlers::java_import;
    LanguageConfig {
        language: tree_sitter_java::LANGUAGE.into(),
        class_kinds: &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "annotation_type_declaration",
        ],
        function_kinds: &["method_declaration", "constructor_declaration"],
        import_kinds: &["import_declaration"],
        call_kinds: &["method_invocation", "object_creation_expression"],
        name_field: "name",
        import_handler: Some(java_import),
    }
}

#[cfg(feature = "lang-c")]
fn c() -> LanguageConfig {
    use super::import_handlers::c_include;
    LanguageConfig {
        language: tree_sitter_c::LANGUAGE.into(),
        // C has no class-like constructs; struct handling is separate
        class_kinds: &[],
        function_kinds: &["function_definition"],
        import_kinds: &["preproc_include"],
        call_kinds: &["call_expression"],
        // C function names are nested inside a declarator chain
        name_field: "declarator",
        import_handler: Some(c_include),
    }
}

#[cfg(feature = "lang-cpp")]
fn cpp() -> LanguageConfig {
    use super::import_handlers::c_include;
    LanguageConfig {
        language: tree_sitter_cpp::LANGUAGE.into(),
        class_kinds: &["class_specifier", "struct_specifier"],
        function_kinds: &["function_definition"],
        import_kinds: &["preproc_include"],
        call_kinds: &["call_expression"],
        name_field: "declarator",
        import_handler: Some(c_include),
    }
}

#[cfg(feature = "lang-csharp")]
fn csharp() -> LanguageConfig {
    use super::import_handlers::csharp_using;
    LanguageConfig {
        language: tree_sitter_c_sharp::LANGUAGE.into(),
        class_kinds: &[
            "class_declaration",
            "interface_declaration",
            "struct_declaration",
            "enum_declaration",
            "record_declaration",
        ],
        function_kinds: &[
            "method_declaration",
            "constructor_declaration",
            "local_function_statement",
        ],
        import_kinds: &["using_directive"],
        call_kinds: &["invocation_expression"],
        name_field: "name",
        import_handler: Some(csharp_using),
    }
}
