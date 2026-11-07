use crate::block::Classified;
use std::path::Path;
use tree_sitter::Node;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LanguageId {
    Python, Cpp, C, JavaScript, TypeScript, Rust, Kdl, Plain,
}

impl LanguageId {
    pub fn detect(path: &Path) -> Self {
        match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
        {
            "py" => Self::Python,
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Self::Cpp,
            "c" | "h" => Self::C,
            "js" | "mjs" | "jsx" => Self::JavaScript,
            "ts" | "tsx" => Self::TypeScript,
            "rs" => Self::Rust,
            "kdl" => Self::Kdl,
            _ => Self::Plain,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Python => "Python",
            Self::Cpp => "C++",
            Self::C => "C",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Rust => "Rust",
            Self::Kdl => "KDL",
            Self::Plain => "Plain text",
        }
    }

    pub fn line_comment(&self) -> Option<&'static str> {
        match self {
            Self::Python => Some("#"),
            Self::Cpp | Self::C | Self::JavaScript | Self::TypeScript | Self::Rust | Self::Kdl => Some("//"),
            Self::Plain => None,
        }
    }

    pub fn groups(&self) -> &'static [&'static str] {
        match self {
            Self::Python => &["Imports", "Classes", "Functions", "Variables", "Constants", "Decorators", "Main", "Comments", "Disabled", "Errors"],
            Self::Cpp => &["Includes", "Macros", "Namespaces", "Classes", "Functions", "Constructors", "Variables", "Enums", "Templates", "Typedefs", "Comments", "Disabled", "Errors"],
            Self::C => &["Includes", "Macros", "Structs", "Enums", "Typedefs", "Functions", "Prototypes", "Global variables", "Comments", "Disabled", "Errors"],
            Self::JavaScript => &["Imports", "Exports", "Classes", "Functions", "Arrow functions", "Variables", "Comments", "Disabled", "Errors"],
            Self::TypeScript => &["Imports", "Exports", "Classes", "Functions", "Arrow functions", "Variables", "Interfaces", "Types", "Enums", "Comments", "Disabled", "Errors"],
            Self::Rust => &["Imports", "Modules", "Structs", "Enums", "Traits", "Implementations", "Functions", "Constants", "Macros", "Types", "Comments", "Disabled", "Errors"],
            Self::Kdl => &["Binds", "Input", "Outputs", "Layout", "Window rules", "Layer rules", "Animations", "Startup", "Workspaces", "Gestures", "Overview", "Cursor", "Misc", "Comments", "Disabled"],
            Self::Plain => &["Misc"],
        }
    }

    pub fn syntax_ext(&self) -> &'static str {
        match self {
            Self::Python => "py",
            Self::Cpp => "cpp",
            Self::C => "c",
            Self::JavaScript => "js",
            Self::TypeScript => "ts",
            Self::Rust => "rs",
            Self::Kdl => "txt",
            Self::Plain => "txt",
        }
    }

    pub fn ts_language(&self) -> Option<tree_sitter::Language> {
        match self {
            Self::Python => Some(tree_sitter_python::language()),
            Self::Cpp => Some(tree_sitter_cpp::language()),
            Self::C => Some(tree_sitter_c::language()),
            Self::JavaScript => Some(tree_sitter_javascript::language()),
            Self::TypeScript => Some(tree_sitter_typescript::language_typescript()),
            Self::Rust => Some(tree_sitter_rust::language()),
            Self::Kdl | Self::Plain => None,
        }
    }

    pub fn classify(&self, node: Node, bytes: &[u8]) -> Option<Classified> {
        if node.is_error() || node.is_missing() || node.kind() == "ERROR" {
            return Some(Classified {
                group: "Errors",
                title: node.kind().to_string(),
                        sub: text_of(node, bytes).lines().next().unwrap_or("").trim().chars().take(60).collect(),
            });
        }
        match self {
            Self::Python => py_classify(node, bytes),
            Self::Cpp => cpp_classify(node, bytes),
            Self::C => c_classify(node, bytes),
            Self::JavaScript => js_classify(node, bytes, false),
            Self::TypeScript => js_classify(node, bytes, true),
            Self::Rust => rust_classify(node, bytes),
            Self::Kdl | Self::Plain => None,
        }
    }
}

fn text_of(node: Node, bytes: &[u8]) -> String {
    let r = node.byte_range();
    if r.start > r.end || r.end > bytes.len() {
        return String::new();
    }
    String::from_utf8_lossy(&bytes[r.start..r.end]).into_owned()
}

fn name_of(node: Node, bytes: &[u8]) -> Option<String> {
    node.child_by_field_name("name")
    .map(|n| text_of(n, bytes))
}

fn title_sub(node: Node, bytes: &[u8], fallback_title: &str) -> (String, String) {
    let title = name_of(node, bytes).unwrap_or_else(|| fallback_title.to_string());
    let first = text_of(node, bytes);
    let line = first.lines().next().unwrap_or("").trim();
    let sub = line.chars().take(90).collect::<String>();
    (title, sub)
}

fn py_classify(node: Node, bytes: &[u8]) -> Option<Classified> {
    let group = match node.kind() {
        "import_statement" | "import_from_statement" => "Imports",
        "class_definition" => "Classes",
        "function_definition" => "Functions",
        "decorated_definition" => "Functions",
        "assignment" | "augmented_assignment" => {
            let text = text_of(node, bytes);
            let lhs = text.split('=').next().unwrap_or("").trim();
            let is_upper = lhs.chars().all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit());
            if is_upper && !lhs.is_empty() { "Constants" } else { "Variables" }
        }
        "shebang" | "comment" => {
            let text = text_of(node, bytes);
            let row = node.start_position().row;
            // `#!/usr/bin/env python3` and `# -*- coding: utf-8 -*-` are directives, not comments
            // shebang = `#!` with nothing but blank lines / a BOM before it
            let only_blank_before = String::from_utf8_lossy(&bytes[..node.start_byte().min(bytes.len())])
            .trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
            .is_empty();
            let directive = if only_blank_before && text.starts_with("#!") {
                Some("shebang")
            } else if row < 2 && (text.contains("coding:") || text.contains("coding=")) {
                Some("encoding")
            } else {
                None
            };
            if let Some(title) = directive {
                return Some(Classified {
                    group: "Misc",
                    title: title.into(),
                            sub: text.trim().chars().take(90).collect(),
                });
            }
            if text.contains("def ") || text.contains("class ") || text.contains("import ") || text.contains("if __name__") {
                "Disabled"
            } else {
                "Comments"
            }
        }
        "expression_statement" | "if_statement" => {
            let text = text_of(node, bytes);
            if text.starts_with("if __name__") { "Main" } else { "Misc" }
        }
        _ => "Misc",
    };
    let (title, sub) = title_sub(node, bytes, node.kind());
    Some(Classified { group, title, sub })
}

fn cpp_classify(node: Node, bytes: &[u8]) -> Option<Classified> {
    let group = match node.kind() {
        "preproc_include" => "Includes",
        "preproc_def" | "preproc_function_def" | "preproc_call" => "Macros",
        "namespace_definition" => "Namespaces",
        "class_specifier" | "struct_specifier" | "union_specifier" => "Classes",
        "function_definition" => "Functions",
        "enum_specifier" => "Enums",
        "template_declaration" => "Templates",
        "alias_declaration" | "type_definition" => "Typedefs",
        "declaration" => "Variables",
        "comment" => {
            let text = text_of(node, bytes);
            if text.contains("class ") || text.contains("struct ") || text.contains("void ") || text.contains("int ") || text.contains("namespace ") {
                "Disabled"
            } else {
                "Comments"
            }
        }
        _ => "Misc",
    };
    let (title, sub) = title_sub(node, bytes, node.kind());
    Some(Classified { group, title, sub })
}

fn c_classify(node: Node, bytes: &[u8]) -> Option<Classified> {
    let group = match node.kind() {
        "preproc_include" => "Includes",
        "preproc_def" | "preproc_function_def" => "Macros",
        "struct_specifier" | "union_specifier" => "Structs",
        "enum_specifier" => "Enums",
        "type_definition" => "Typedefs",
        "function_definition" => "Functions",
        "declaration" => {
            let text = text_of(node, bytes);
            if text.trim_end().ends_with(';') && text.contains('(') { "Prototypes" } else { "Global variables" }
        }
        "comment" => {
            let text = text_of(node, bytes);
            if text.contains("void ") || text.contains("int ") || text.contains("struct ") || text.contains("enum ") {
                "Disabled"
            } else {
                "Comments"
            }
        }
        _ => "Misc",
    };
    let (title, sub) = title_sub(node, bytes, node.kind());
    Some(Classified { group, title, sub })
}

fn js_classify(node: Node, bytes: &[u8], ts: bool) -> Option<Classified> {
    let group = match node.kind() {
        "import_statement" => "Imports",
        "export_statement" => "Exports",
        "class_declaration" | "abstract_class_declaration" => "Classes",
        "function_declaration" | "generator_function_declaration" => "Functions",
        "lexical_declaration" | "variable_declaration" => {
            let text = text_of(node, bytes);
            if text.contains("=>") { "Arrow functions" } else { "Variables" }
        }
        "comment" => {
            let text = text_of(node, bytes);
            if text.contains("function ") || text.contains("class ") || text.contains("const ") || text.contains("let ") || text.contains("var ") || text.contains("interface ") {
                "Disabled"
            } else {
                "Comments"
            }
        }
        _ if ts && node.kind() == "interface_declaration" => "Interfaces",
        _ if ts && node.kind() == "type_alias_declaration" => "Types",
        _ if ts && node.kind() == "enum_declaration" => "Enums",
        _ => "Misc",
    };
    let (title, sub) = title_sub(node, bytes, node.kind());
    Some(Classified { group, title, sub })
}

fn rust_classify(node: Node, bytes: &[u8]) -> Option<Classified> {
    let group = match node.kind() {
        "use_declaration" | "extern_crate_declaration" => "Imports",
        "mod_item" => "Modules",
        "struct_item" => "Structs",
        "enum_item" => "Enums",
        "trait_item" => "Traits",
        "impl_item" => "Implementations",
        "function_item" => "Functions",
        "const_item" | "static_item" => "Constants",
        "macro_definition" => "Macros",
        "type_item" => "Types",
        "line_comment" | "block_comment" => {
            let text = text_of(node, bytes);
            if text.contains("fn ") || text.contains("struct ") || text.contains("impl ") || text.contains("use ") || text.contains("const ") || text.contains("mod ") {
                "Disabled"
            } else {
                "Comments"
            }
        }
        _ => "Misc",
    };
    let (title, sub) = title_sub(node, bytes, node.kind());
    Some(Classified { group, title, sub })
}
