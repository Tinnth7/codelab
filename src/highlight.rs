use crate::parse::LanguageId;
use egui::text::LayoutJob;
use egui::{Color32, FontId, TextFormat};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use syntect::parsing::SyntaxSet;

/// (byte start, byte end, color, italic) — offsets into the highlighted text.
type Span = (usize, usize, Color32, bool);

struct Cached {
    key: u64,
    spans: Vec<Span>,
}

pub struct Highlighting {
    pub syntax_set: Arc<SyntaxSet>,
    pub theme: Arc<Theme>,
    /// Last highlight result. Re-running syntect every frame is what made big
    /// files unusable, so we only redo it when the text actually changes.
    cache: Arc<Mutex<Option<Cached>>>,
}

fn c(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

fn rule(scope: &str, fg: Color, font: Option<FontStyle>) -> ThemeItem {
    ThemeItem {
        scope: ScopeSelectors::from_str(scope).expect("bad scope selector"),
        style: StyleModifier { foreground: Some(fg), background: None, font_style: font },
    }
}

/// One Dark-ish palette.
fn one_dark() -> Theme {
    let gray = c(92, 99, 112);
    let green = c(152, 195, 121);
    let orange = c(209, 154, 102);
    let purple = c(198, 120, 221);
    let cyan = c(86, 182, 194);
    let blue = c(97, 175, 239);
    let yellow = c(229, 192, 123);
    let red = c(224, 108, 117);

    Theme {
        name: Some("codelab-one-dark".into()),
        author: None,
        settings: ThemeSettings { foreground: Some(c(171, 178, 191)), ..Default::default() },
        scopes: vec![
            rule("comment", gray, Some(FontStyle::ITALIC)),
            rule("string, constant.character", green, None),
            rule("constant.numeric, constant.language, constant.other, support.constant", orange, None),
            rule("keyword, storage", purple, None),
            rule("keyword.operator", cyan, None),
            rule("entity.name.function, support.function, variable.function, meta.function-call", blue, None),
            rule("support.function.macro, entity.name.function.macro, support.macro", cyan, None),
            rule("entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, entity.name.trait, entity.name.namespace, support.type, support.class, entity.other.inherited-class", yellow, None),
            rule("variable.parameter, variable.other.member, variable.other.property, variable.language", red, None),
        ],
    }
}

impl Highlighting {
    pub fn new() -> Self {
        Self {
            syntax_set: Arc::new(SyntaxSet::load_defaults_newlines()),
            theme: Arc::new(one_dark()),
            cache: Arc::new(Mutex::new(None)),
        }
    }

    pub fn layouter<'a>(&'a self, lang: LanguageId, font_size: f32) -> impl FnMut(&egui::Ui, &str, f32) -> Arc<egui::Galley> + 'a {
        let syntax_set = self.syntax_set.clone();
        let theme = self.theme.clone();
        let cache = self.cache.clone();
        let ext = lang.syntax_ext().to_string();
        move |ui: &egui::Ui, text: &str, wrap_width: f32| {
            let mut job = LayoutJob::default();
            job.wrap.max_width = if wrap_width.is_finite() && wrap_width > 0.0 { wrap_width } else { f32::INFINITY };
            let font_id = FontId::monospace(font_size);

            let mut hasher = DefaultHasher::new();
            ext.hash(&mut hasher);
            text.hash(&mut hasher);
            let key = hasher.finish();

            let mut guard = cache.lock().unwrap();
            let fresh = match guard.as_ref() {
                Some(cached) => cached.key != key,
                None => true,
            };
            if fresh {
                let syntax = syntax_set
                .find_syntax_by_extension(&ext)
                .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
                let default_fg = theme.settings.foreground.unwrap_or(c(171, 178, 191));
                let default_color = Color32::from_rgb(default_fg.r, default_fg.g, default_fg.b);

                let mut hl = HighlightLines::new(syntax, &theme);
                let mut spans: Vec<Span> = Vec::new();
                let mut offset = 0usize;
                for line in text.split_inclusive('\n') {
                    match hl.highlight_line(line, &syntax_set) {
                        Ok(ranges) => {
                            for (style, piece) in ranges {
                                let fg = style.foreground;
                                let color = Color32::from_rgb(fg.r, fg.g, fg.b);
                                let italic = style.font_style.contains(FontStyle::ITALIC);
                                let end = offset + piece.len();
                                // merge neighbours with identical style -> far fewer layout sections
                                match spans.last_mut() {
                                    Some(last) if last.1 == offset && last.2 == color && last.3 == italic => last.1 = end,
                                    _ => spans.push((offset, end, color, italic)),
                                }
                                offset = end;
                            }
                        }
                        Err(_) => {
                            // never drop text if syntect fails on a line: show it uncolored
                            let end = offset + line.len();
                            spans.push((offset, end, default_color, false));
                            offset = end;
                        }
                    }
                }
                *guard = Some(Cached { key, spans });
            }

            if let Some(cached) = guard.as_ref() {
                for &(a, b, color, italic) in &cached.spans {
                    let fmt = TextFormat { font_id: font_id.clone(), color, italics: italic, ..Default::default() };
                    job.append(&text[a..b], 0.0, fmt);
                }
            }
            drop(guard);
            ui.fonts(|f| f.layout_job(job))
        }
    }
}
