use crate::block::Block;
use crate::parse::LanguageId;
use std::ops::Range;

pub fn split(source: &str, lang: LanguageId) -> Vec<Block> {
    match lang {
        LanguageId::Kdl => split_kdl(source),
        LanguageId::Plain => split_plain(source),
        _ => split_tree_sitter(source, lang),
    }
}

fn split_plain(source: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut pos = 0;
    for line in source.split_inclusive('\n') {
        if line.trim().is_empty() && start < pos {
            let text = &source[start..pos];
            if !text.trim().is_empty() {
                out.push(make_plain_block(start..pos, text));
            }
            start = pos;
        }
        pos += line.len();
    }
    if start < source.len() && !source[start..].trim().is_empty() {
        out.push(make_plain_block(start..source.len(), &source[start..]));
    }
    out
}

fn make_plain_block(range: Range<usize>, text: &str) -> Block {
    let title = text.lines().next().unwrap_or("").trim().chars().take(60).collect();
    Block {
        range, group: "Misc".into(), title, sub: String::new(), disabled: false, in_binds: false,
    }
}

fn split_tree_sitter(source: &str, lang: LanguageId) -> Vec<Block> {
    let Some(ts_lang) = lang.ts_language() else { return split_plain(source) };
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&ts_lang).is_err() {
        warn!("tree-sitter grammar for {} failed to load; using plain-text blocks", lang.name());
        return split_plain(source);
    }
    let Some(tree) = parser.parse(source, None) else {
        warn!("tree-sitter could not parse {}; using plain-text blocks", lang.name());
        return split_plain(source);
    };
    let root = tree.root_node();
    let bytes = source.as_bytes();
    let mut blocks = Vec::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.is_named() {
            if let Some(c) = lang.classify(child, bytes) {
                blocks.push(Block {
                    range: child.byte_range(),
                            group: c.group.into(),
                            title: c.title,
                            sub: c.sub,
                            disabled: false,
                            in_binds: false,
                });
            } else {
                let r = child.byte_range();
                if r.start < r.end {
                    blocks.push(Block {
                        range: r.clone(),
                                group: "Misc".into(),
                                title: crate::block::first_line(&source[r.clone()]).chars().take(60).collect(),
                                sub: String::new(),
                                disabled: false,
                                in_binds: false,
                    });
                }
            }
        }
    }
    blocks
}

// ---------- KDL manual scanner ----------

fn split_kdl(source: &str) -> Vec<Block> {
    let ranges = scan_top_level(source);
    let mut blocks = Vec::new();
    for r in ranges {
        let text = &source[r.clone()];
        let first_tok = text.trim_start()
        .split(|c: char| c.is_whitespace() || c == '{' || c == ';')
        .next().unwrap_or("");
        if first_tok == "binds" || first_tok == "/-binds" {
            if let (Some(open), Some(close)) = (text.find('{'), text.rfind('}')) {
                let inner_start = r.start + open + 1;
                let inner_end = r.start + close;
                if inner_start < inner_end {
                    let inner = &source[inner_start..inner_end];
                    for ir in scan_top_level(inner) {
                        let br = (inner_start + ir.start)..(inner_start + ir.end);
                        let bt = &source[br.clone()];
                        if bt.trim().is_empty() { continue; }
                        let (title, sub) = extract_bind_title(bt);
                        let disabled = bt.trim_start().starts_with("/-");
                        blocks.push(Block {
                            range: br, group: "Binds".into(), title, sub, disabled, in_binds: true,
                        });
                    }
                    continue;
                }
            }
        }
        let (group, title, sub) = classify_kdl(text);
        let disabled = text.trim_start().starts_with("/-");
        blocks.push(Block { range: r, group: group.into(), title, sub, disabled, in_binds: false });
    }
    blocks
}

fn classify_kdl(text: &str) -> (&'static str, String, String) {
    let trimmed = text.trim_start();
    let stripped = trimmed.strip_prefix("/-").unwrap_or(trimmed);
    let first_tok: String = stripped
    .split(|c: char| c.is_whitespace() || c == '{' || c == ';' || c == '"')
    .next().unwrap_or("").to_string();

    let group = match first_tok.as_str() {
        "binds" => "Binds",
        "input" => "Input",
        "output" => "Outputs",
        "layout" => "Layout",
        "window-rule" => "Window rules",
        "layer-rule" => "Layer rules",
        "animations" => "Animations",
        "spawn-at-startup" | "spawn-sh-at-startup" | "environment" | "prefer-no-csd" | "screenshot-path" => "Startup",
        "workspace" => "Workspaces",
        "gestures" => "Gestures",
        "overview" => "Overview",
        "cursor" => "Cursor",
        "" if stripped.trim_start().starts_with("//") => "Comments",
        _ => "Misc",
    };

    let first_line = stripped.lines().next().unwrap_or("").trim();
    let title = first_tok.clone();
    let sub = first_line
    .strip_prefix(&first_tok).unwrap_or("")
    .trim()
    .chars().take(90).collect();

    (group, title, sub)
}

fn extract_bind_title(text: &str) -> (String, String) {
    let trimmed = text.trim_start();
    let stripped = trimmed.strip_prefix("/-").unwrap_or(trimmed);
    let first_line = stripped.lines().next().unwrap_or("").trim();
    let title_end = first_line.find(|c: char| c == '{' || c == ';' || c.is_whitespace()).unwrap_or(first_line.len());
    let title = first_line[..title_end].to_string();
    let sub = if let (Some(o), Some(c)) = (stripped.find('{'), stripped.rfind('}')) {
        if o < c {
            stripped[o + 1..c].trim().chars().take(90).collect()
        } else { String::new() }
    } else { String::new() };
    (title, sub)
}

fn scan_top_level(source: &str) -> Vec<Range<usize>> {
    let bytes = source.as_bytes();
    let n = bytes.len();
    let mut out: Vec<Range<usize>> = Vec::new();
    let mut i = 0;
    let mut start = 0;
    let mut depth: i32 = 0;
    let mut line_start = 0;
    let mut code = false;

    while i < n {
        let c = bytes[i];
        let c2 = if i + 1 < n { bytes[i + 1] } else { 0 };

        if c == b'/' && c2 == b'/' {
            while i < n && bytes[i] != b'\n' { i += 1; }
            continue;
        }
        if c == b'/' && c2 == b'*' {
            i += 2;
            while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') { i += 1; }
            i = (i + 2).min(n);
            continue;
        }
        if c == b'/' && c2 == b'-' {
            i += 2;
            continue;
        }
        if c == b'"' {
            i += 1;
            while i < n && bytes[i] != b'"' {
                if bytes[i] == b'\\' { i += 1; }
                i += 1;
            }
            i += 1;
            code = true;
            continue;
        }
        if c == b'r' {
            let mut hashes = 0;
            let mut j = i + 1;
            while j < n && bytes[j] == b'#' { hashes += 1; j += 1; }
            let prev_ok = i == 0 || matches!(bytes[i - 1], b' ' | b'\t' | b'\n' | b'{' | b';');
            if j < n && bytes[j] == b'"' && prev_ok {
                j += 1;
                let mut terminator = vec![b'"'];
                for _ in 0..hashes { terminator.push(b'#'); }
                loop {
                    if j + terminator.len() > n { break; }
                    if bytes[j..j + terminator.len()] == terminator[..] {
                        j += terminator.len();
                        break;
                    }
                    j += 1;
                }
                i = j;
                code = true;
                continue;
            }
        }
        if c == b'{' { depth += 1; code = true; }
        else if c == b'}' { depth -= 1; code = true; }
        else if c == b'\n' {
            if depth == 0 {
                if code {
                    out.push(start..(i + 1));
                    start = i + 1;
                    code = false;
                } else if line_start < i
                    && source[line_start..i].trim().is_empty()
                    && !source[start..i].trim().is_empty()
                    {
                        out.push(start..(i + 1));
                        start = i + 1;
                    }
            }
            line_start = i + 1;
        } else if !c.is_ascii_whitespace() {
            code = true;
        }
        i += 1;
    }
    if start < n && !source[start..].trim().is_empty() {
        out.push(start..n);
    }
    out
}
