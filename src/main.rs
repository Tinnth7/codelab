#[macro_use]
mod logger;
mod block;
mod capture;
mod highlight;
mod parse;
mod splitter;

use block::{find_near, Block};
use capture::CaptureState;
use egui::{Color32, Context, FontId, Key, RichText, ScrollArea, TextEdit, Ui};
use highlight::Highlighting;
use parse::LanguageId;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const REPO_URL: &str = "https://github.com/Tinnth7/codelab";

const SHORTCUTS: &[(&str, &str)] = &[
    ("Ctrl+S", "Save"),
    ("Ctrl+Shift+S", "Save As\u{2026}"),
    ("Ctrl+O", "Open file"),
    ("Ctrl+N", "New block"),
    ("Ctrl+D", "Delete block"),
    ("Ctrl+E", "Toggle disabled"),
    ("Ctrl+F", "Find blocks"),
    ("Ctrl+`", "Toggle terminal"),
    ("Ctrl+K", "Capture key (KDL binds)"),
];

/// Syntax highlighting is cached, so only truly huge buffers skip it.
const HL_MAX_BYTES: usize = 500_000;

/// Lucide icon codepoints (assets/lucide.ttf, from lucide-static).
mod icons {
    pub const SAVE: &str = "\u{e14d}"; // save
    pub const PLUS: &str = "\u{e13d}"; // plus
    pub const TRASH: &str = "\u{e18e}"; // trash-2
    pub const BAN: &str = "\u{e051}"; // ban
    pub const CIRCLE_CHECK: &str = "\u{e226}"; // circle-check
    pub const KEYBOARD: &str = "\u{e284}"; // keyboard
    pub const FILES: &str = "\u{e0cf}"; // files
    pub const GIT_BRANCH: &str = "\u{e0e2}"; // git-branch
    pub const REFRESH: &str = "\u{e145}"; // refresh-cw
    pub const X: &str = "\u{e1b2}"; // x
    pub const TERMINAL: &str = "\u{e181}"; // terminal
}

fn lucide_family() -> egui::FontFamily {
    egui::FontFamily::Name("lucide".into())
}

/// Lucide glyphs sit high in their em box, so icons looked ~2px above the label text.
/// This shifts them down by a fraction of the font size; tweak if it still looks off.
const ICON_Y_OFFSET: f32 = 0.16;

fn install_icon_font(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "lucide".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/lucide.ttf"))
            .tweak(egui::FontTweak { y_offset_factor: ICON_Y_OFFSET, ..Default::default() })
            .into(),
    );
    fonts.families.insert(lucide_family(), vec!["lucide".to_owned()]);

    // JetBrains Mono goes first in the monospace family (editor, gutter, terminal);
    // egui's built-in monospace font stays behind it as a glyph fallback.
    fonts.font_data.insert(
        "jetbrains-mono".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/JetBrainsMono-Regular.ttf")).into(),
    );
    fonts.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "jetbrains-mono".to_owned());

    ctx.set_fonts(fonts);
}

/// Lucide glyph + optional text label, colored by the button's own state (hover/disabled).
fn icon_label(icon: &str, text: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        icon,
        0.0,
        egui::TextFormat {
            font_id: FontId::new(15.0, lucide_family()),
            color: Color32::PLACEHOLDER,
            valign: egui::Align::Center,
            ..Default::default()
        },
    );
    if !text.is_empty() {
        job.append(
            text,
            6.0,
            egui::TextFormat {
                font_id: FontId::proportional(14.0),
                color: Color32::PLACEHOLDER,
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
    }
    job
}

/// Small caps title row used by every panel; `right` fills the right-aligned slot.
fn panel_header(ui: &mut Ui, title: &str, right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(26.0);
        ui.add_space(2.0);
        ui.label(RichText::new(title).small().strong().color(ui.visuals().weak_text_color()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    });
    ui.separator();
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let initial_path = args.get(1).cloned().unwrap_or_default();

    std::panic::set_hook(Box::new(|info| err!("panic: {info}")));
    info!("Codelab {} starting", env!("CARGO_PKG_VERSION"));
    if !initial_path.is_empty() {
        info!("opening {initial_path}");
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
        .with_inner_size([1350.0, 820.0])
        .with_min_inner_size([800.0, 500.0])
        .with_app_id("codelab")
        .with_icon(egui::IconData {
            rgba: include_bytes!("../assets/icon_256.rgba").to_vec(),
            width: 256,
            height: 256,
        }),
        ..Default::default()
    };
    let result = eframe::run_native(
        "Codelab",
        options,
        Box::new(move |cc| Ok(Box::new(Lab::new(cc, initial_path)))),
    );
    match &result {
        Ok(()) => info!("Codelab exited"),
        Err(e) => err!("fatal: {e}"),
    }
    result
}

#[derive(Serialize, Deserialize)]
struct PersistedState {
    dark: bool,
    hide_c: bool,
    autosave: bool,
    backup_dir: Option<PathBuf>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self { dark: true, hide_c: false, autosave: true, backup_dir: None }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum StatusKind { Info, Busy, Ok, Error }

impl StatusKind {
    fn color(&self) -> Color32 {
        match self {
            Self::Info => Color32::GRAY,
            Self::Busy => Color32::from_rgb(220, 180, 60),
            Self::Ok => Color32::from_rgb(80, 200, 120),
            Self::Error => Color32::from_rgb(220, 80, 80),
        }
    }
    fn prefix(&self) -> &'static str {
        match self {
            Self::Info => "",
            Self::Busy => "… ",
            Self::Ok => "✓ ",
            Self::Error => "! ",
        }
    }
}

#[derive(Copy, Clone)]
enum CaptureTarget { New, Recapture(usize) }

#[derive(Copy, Clone, PartialEq, Eq)]
enum SideTab { Explorer, Git }


pub struct Lab {
    path: Option<PathBuf>,
    lang: LanguageId,
    source: String,
    blocks: Vec<Block>,

    explorer_root: Option<PathBuf>,
    explorer_tree: Vec<FileNode>,

    cur: String,
    search: String,
    expanded: std::collections::HashSet<String>,

    sel: Option<usize>,
    tab: usize,
    raw_buf: String,

    raw_dirty_since: Option<Instant>,
    block_dirty_since: Option<Instant>,

    capture: Option<CaptureState>,
    capture_target: CaptureTarget,

    dark: bool,
    hide_c: bool,
    autosave: bool,
    backup_dir: Option<PathBuf>,
    last_autosave: Option<Instant>,

    status_msg: Option<String>,
    status_kind: StatusKind,
    status_until: Option<Instant>,

    dirty: bool,
    show_confirm_delete: Option<usize>,
    show_about: bool,
    show_shortcuts: bool,

    side_tab: SideTab,
    side_open: bool,
    term_open: bool,

    hl: Highlighting,
}

impl Lab {
    fn new(cc: &eframe::CreationContext<'_>, initial_path: String) -> Self {
        install_icon_font(&cc.egui_ctx);
        let persisted = cc.storage
        .and_then(|s| s.get_string("codelab_state"))
        .and_then(|s| serde_json::from_str::<PersistedState>(&s).ok())
        .unwrap_or_default();

        if persisted.dark { cc.egui_ctx.set_visuals(egui::Visuals::dark()); }
        else { cc.egui_ctx.set_visuals(egui::Visuals::light()); }

        let mut lab = Lab {
            path: None,
            lang: LanguageId::Plain,
            source: String::new(),
            blocks: Vec::new(),
            explorer_root: None,
            explorer_tree: Vec::new(),
            cur: "All".into(),
            search: String::new(),
            expanded: Default::default(),
            sel: None,
            tab: 0,
            raw_buf: String::new(),
            raw_dirty_since: None,
            block_dirty_since: None,
            capture: None,
            capture_target: CaptureTarget::New,
            dark: persisted.dark,
            hide_c: persisted.hide_c,
            autosave: persisted.autosave,
            backup_dir: persisted.backup_dir,
            last_autosave: None,
            status_msg: None,
            status_kind: StatusKind::Info,
            status_until: None,
            dirty: false,
            show_confirm_delete: None,
            show_about: false,
            show_shortcuts: false,
            side_tab: SideTab::Explorer,
            side_open: true,
            term_open: false,
            hl: Highlighting::new(),
        };

        if !initial_path.is_empty() && Path::new(&initial_path).exists() {
            if Path::new(&initial_path).is_dir() {
                lab.load_folder(&initial_path);
            } else {
                lab.load(&initial_path);
            }
        }
        lab
    }

    fn load(&mut self, path: &str) {
        match fs::read_to_string(path) {
            Ok(text) => {
                self.lang = LanguageId::detect(Path::new(path));
                if self.lang == LanguageId::Plain {
                    warn!("no syntax support for {path}; treating it as plain text");
                }
                self.source = text;
                self.path = Some(PathBuf::from(path));
                self.reparse();
                self.dirty = false;
                self.raw_dirty_since = None;
                self.block_dirty_since = None;
                self.status(format!("Opened {} ({})", path, self.lang.name()), StatusKind::Info);
            }
            Err(e) => self.status(format!("Open failed: {e}"), StatusKind::Error),
        }
    }

    fn load_folder(&mut self, path: &str) {
        let p = PathBuf::from(path);
        self.explorer_root = Some(p.clone());
        self.explorer_tree = scan_tree(&p);
        self.status(format!("Loaded folder {}", path), StatusKind::Info);
    }

    fn reparse(&mut self) {
        self.blocks = splitter::split(&self.source, self.lang);
        if let Some(i) = self.sel {
            if i >= self.blocks.len() { self.sel = self.blocks.first().map(|_| 0); }
        } else if !self.blocks.is_empty() {
            self.sel = Some(0);
        }
        self.refresh_raw_if_on_tab();
    }

    fn refresh_raw_if_on_tab(&mut self) {
        if self.tab == 1 && self.raw_dirty_since.is_none() {
            self.raw_buf = self.source.clone();
        }
    }

    fn editor_buffer(&self) -> String {
        if self.tab == 1 { self.raw_buf.clone() } else { self.source.clone() }
    }

    fn status(&mut self, msg: impl Into<String>, kind: StatusKind) {
        let msg = msg.into();
        match kind {
            StatusKind::Error => err!("{msg}"),
            StatusKind::Info | StatusKind::Ok if msg != "Synced" => info!("{msg}"),
            _ => {}
        }
        self.status_msg = Some(msg);
        self.status_kind = kind;
        self.status_until = Some(Instant::now() + Duration::from_secs(6));
    }

    fn tick_timers(&mut self, ctx: &Context) {
        let now = Instant::now();

        if let Some(t) = self.raw_dirty_since {
            if now.duration_since(t) >= Duration::from_secs(1) {
                self.source = self.raw_buf.clone();
                self.reparse();
                self.raw_dirty_since = None;
                self.block_dirty_since = None;
                self.status("Synced", StatusKind::Ok);
                self.status_until = Some(now + Duration::from_millis(1200));
            } else {
                ctx.request_repaint_after(Duration::from_secs(1) - now.duration_since(t));
                if self.status_kind != StatusKind::Busy {
                    self.status("⏳ Syncing and rebuilding…", StatusKind::Busy);
                    self.status_until = None;
                }
            }
        }

        if let Some(t) = self.block_dirty_since {
            if now.duration_since(t) >= Duration::from_secs(1) {
                self.block_dirty_since = None;
            } else {
                ctx.request_repaint_after(Duration::from_secs(1) - now.duration_since(t));
            }
        }

        if let Some(until) = self.status_until {
            if now >= until {
                self.status_msg = None;
                self.status_until = None;
            } else {
                ctx.request_repaint_after(until - now);
            }
        }
    }

    fn maybe_autosave(&mut self, ctx: &Context) {
        if !self.autosave || !self.dirty { return; }
        let Some(path) = self.path.clone() else { return; };

        let now = Instant::now();
        if let Some(last) = self.last_autosave {
            if now.duration_since(last) < Duration::from_secs(300) {
                if let Some(next) = last.checked_add(Duration::from_secs(300)) {
                    if next > now { ctx.request_repaint_after(next - now); }
                }
                return;
            }
        }

        let buffer = self.editor_buffer();
        let dir = self.backup_dir_or_default();
        if let Ok(Some(bk)) = write_snapshot(&path, buffer.as_bytes(), &dir) {
            let name = bk.file_name().and_then(|s| s.to_str()).unwrap_or("?");
            self.status(format!("Autosave: {name}"), StatusKind::Ok);
        }
        self.last_autosave = Some(now);
        ctx.request_repaint_after(Duration::from_secs(300));
    }

    fn backup_dir_or_default(&self) -> PathBuf {
        self.backup_dir.clone().unwrap_or_else(|| {
            directories::BaseDirs::new()
            .map(|b| b.data_dir().join("codelab").join("backups"))
            .unwrap_or_else(|| PathBuf::from("backups"))
        })
    }

    fn save(&mut self) -> bool {
        if let Some(_) = self.raw_dirty_since {
            self.source = self.raw_buf.clone();
            self.reparse();
            self.raw_dirty_since = None;
        }

        let Some(path) = self.path.clone() else {
            let Some(f) = rfd::FileDialog::new().set_title("Save file").save_file() else { return false; };
            self.path = Some(f.clone());
            self.lang = LanguageId::detect(&f);
            return self.save_to(&f);
        };
        self.save_to(&path)
    }

    fn save_to(&mut self, path: &Path) -> bool {
        let text = self.editor_buffer();

        if let Ok(existing) = fs::read(path) {
            if existing != text.as_bytes() {
                let dir = self.backup_dir_or_default();
                let _ = write_snapshot(path, &existing, &dir);
            }
        }

        if let Err(e) = fs::write(path, text.as_bytes()) {
            self.status(format!("Save failed: {e}"), StatusKind::Error);
            return false;
        }
        self.dirty = false;
        self.status("Saved", StatusKind::Ok);
        true
    }

    fn save_as(&mut self) -> bool {
        let Some(f) = rfd::FileDialog::new().set_title("Save file as").save_file() else {
            info!("save dialog closed without a selection");
            return false;
        };
        if self.raw_dirty_since.is_some() {
            self.source = self.raw_buf.clone();
            self.raw_dirty_since = None;
        }
        self.path = Some(f.clone());
        self.lang = LanguageId::detect(&f);
        self.reparse();
        self.save_to(&f)
    }

    fn new_block(&mut self) {
        if self.lang == LanguageId::Kdl && self.cur == "Binds" {
            self.start_capture(CaptureTarget::New);
            return;
        }
        let template = match self.cur.as_str() {
            "Window rules" => "window-rule {\n\n}\n",
            "Layer rules" => "layer-rule {\n\n}\n",
            "Outputs" => "output \"\" {\n\n}\n",
            "Workspaces" => "workspace \"\" {\n\n}\n",
            "Startup" => "spawn-at-startup \"app\"\n",
            "Comments" => "// \n",
            _ => "",
        };
        if template.is_empty() { return; }
        let at = if self.source.ends_with('\n') || self.source.is_empty() {
            self.source.len()
        } else {
            self.source.push('\n');
            self.source.len()
        };
        self.source.insert_str(at, template);
        self.reparse();
        if let Some(i) = find_near(&self.blocks, at) { self.sel = Some(i); }
        self.dirty = true;
        self.status(format!("New block in {}", self.cur), StatusKind::Info);
    }

    fn delete_selected(&mut self) {
        let Some(i) = self.sel else { return; };
        let b = &self.blocks[i];
        self.source.replace_range(b.range.clone(), "");
        self.reparse();
        self.sel = self.blocks.first().map(|_| 0);
        self.dirty = true;
        self.status("Block deleted", StatusKind::Info);
    }

    fn toggle_disabled(&mut self) {
        let Some(i) = self.sel else { return; };
        let b = self.blocks[i].clone();
        let text = self.source[b.range.clone()].to_string();
        let old_start = b.range.start;

        if self.lang == LanguageId::Kdl {
            let trimmed_start = text.len() - text.trim_start().len();
            let insert_pos = b.range.start + trimmed_start;
            if text.trim_start().starts_with("/-") {
                self.source.replace_range(insert_pos..insert_pos + 2, "");
            } else {
                self.source.insert_str(insert_pos, "/-");
            }
        } else {
            let comment = self.lang.line_comment().unwrap_or("//");
            let is_disabled = b.group == "Disabled";

            let mut new_text = String::new();
            for line in text.lines() {
                if is_disabled {
                    let trimmed = line.trim_start();
                    let indent = &line[..line.len() - trimmed.len()];
                    if let Some(rest) = trimmed.strip_prefix(comment) {
                        let rest = rest.strip_prefix(' ').unwrap_or(rest);
                        new_text.push_str(&format!("{}{}\n", indent, rest));
                    } else {
                        new_text.push_str(&format!("{}\n", line));
                    }
                } else {
                    if line.trim().is_empty() {
                        new_text.push_str(&format!("{}\n", line));
                    } else {
                        let indent = &line[..line.len() - line.trim_start().len()];
                        let rest = line.trim_start();
                        new_text.push_str(&format!("{}{} {}\n", indent, comment, rest));
                    }
                }
            }
            if !text.ends_with('\n') { new_text.pop(); }
            self.source.replace_range(b.range.clone(), &new_text);
        }

        self.reparse();
        self.sel = find_near(&self.blocks, old_start);
        self.dirty = true;
    }

    fn start_capture(&mut self, target: CaptureTarget) {
        self.capture = Some(CaptureState::new());
        self.capture_target = target;
    }

    fn finish_capture(&mut self, combo: String) {
        match self.capture_target {
            CaptureTarget::New => self.insert_new_bind(&combo),
            CaptureTarget::Recapture(i) => self.recapture_bind(i, &combo),
        }
    }

    fn insert_new_bind(&mut self, combo: &str) {
        let body = format!("{} {{ ; }}", combo);
        let last_bind = self.blocks.iter().filter(|b| b.in_binds).max_by_key(|b| b.range.start);
        if let Some(last) = last_bind {
            let insert_at = last.range.end;
            let line_start = self.source[..last.range.start].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let indent: String = self.source[line_start..last.range.start].chars().take_while(|c| c.is_whitespace()).collect();
            let text = format!("{}{}\n", indent, body);
            self.source.insert_str(insert_at, &text);
        } else {
            if !self.source.ends_with('\n') { self.source.push('\n'); }
            let wrapper = format!("binds {{\n    {}\n}}\n", body);
            self.source.push_str(&wrapper);
        }
        self.reparse();
        if let Some(i) = self.blocks.iter().enumerate().rev().find(|(_, b)| b.in_binds).map(|(i, _)| i) {
            self.sel = Some(i);
        }
        self.dirty = true;
        self.status(format!("Captured {combo}"), StatusKind::Ok);
    }

    fn recapture_bind(&mut self, i: usize, combo: &str) {
        let Some(b) = self.blocks.get(i).cloned() else { return; };
        let text = &self.source[b.range.clone()];
        let trimmed_start = text.len() - text.trim_start().len();
        let stripped = text.trim_start().strip_prefix("/-").unwrap_or(text.trim_start());
        let first_tok_len = stripped
        .split(|c: char| c == '{' || c == ';' || c.is_whitespace())
        .next().map(|s| s.len()).unwrap_or(0);
        if first_tok_len == 0 { return; }
        let key_start = b.range.start + trimmed_start + (text.trim_start().len() - stripped.len());
        self.source.replace_range(key_start..key_start + first_tok_len, combo);
        self.reparse();
        self.dirty = true;
        self.status(format!("Recaptured {combo}"), StatusKind::Ok);
    }

    fn switch_tab(&mut self, new_tab: usize) {
        if self.tab == new_tab { return; }
        if self.tab == 0 && new_tab == 1 {
            self.raw_buf = self.source.clone();
            self.raw_dirty_since = None;
        } else if self.tab == 1 && new_tab == 0 {
            if let Some(_) = self.raw_dirty_since {
                self.source = self.raw_buf.clone();
                self.reparse();
                self.raw_dirty_since = None;
            }
        }
        self.tab = new_tab;
    }

    fn open_dialog(&mut self) {
        if self.dirty {
            let r = rfd::MessageDialog::new()
            .set_title("Unsaved changes")
            .set_description("Discard current changes?")
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
            if r != rfd::MessageDialogResult::Yes { return; }
        }
        match rfd::FileDialog::new().pick_file() {
            Some(f) => self.load(f.to_str().unwrap_or("")),
            None => info!("file dialog closed without a selection (cancelled, or no file-chooser portal available)"),
        }
    }

    fn open_folder_dialog(&mut self) {
        match rfd::FileDialog::new().pick_folder() {
            Some(f) => self.load_folder(f.to_str().unwrap_or("")),
            None => info!("folder dialog closed without a selection (cancelled, or no file-chooser portal available)"),
        }
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(icon_label(icons::SAVE, "Save"))).on_hover_text("Save (Ctrl+S)").clicked() {
                self.save();
            }
            ui.separator();
            let has_sel = self.sel.is_some();
            if ui.add_enabled(has_sel, egui::Button::new(icon_label(icons::PLUS, "New"))).on_hover_text("New block (Ctrl+N)").clicked() {
                self.new_block();
            }
            if ui.add_enabled(has_sel, egui::Button::new(icon_label(icons::TRASH, "Delete"))).on_hover_text("Delete block (Ctrl+D)").clicked() {
                self.show_confirm_delete = self.sel;
            }
            let is_disabled = self.sel.and_then(|i| self.blocks.get(i)).map(|b| b.group == "Disabled" || b.disabled).unwrap_or(false);
            let (dis_icon, dis_label) = if is_disabled { (icons::CIRCLE_CHECK, "Enable") } else { (icons::BAN, "Disable") };
            if ui.add_enabled(has_sel, egui::Button::new(icon_label(dis_icon, dis_label))).on_hover_text("Toggle disabled (Ctrl+E)").clicked() {
                self.toggle_disabled();
            }
            if self.lang == LanguageId::Kdl {
                if ui.add_enabled(has_sel, egui::Button::new(icon_label(icons::KEYBOARD, "Capture"))).on_hover_text("Capture key (Ctrl+K)").clicked() {
                    if let Some(i) = self.sel {
                        if self.blocks[i].in_binds { self.start_capture(CaptureTarget::Recapture(i)); }
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let term_btn = egui::Button::new(icon_label(icons::TERMINAL, "Terminal")).selected(self.term_open);
                if ui.add(term_btn).on_hover_text("Toggle terminal (Ctrl+`)").clicked() {
                    self.toggle_terminal(ui.ctx());
                }
                ui.checkbox(&mut self.hide_c, "Hide comments");
            });
        });
    }

    // ---------- activity bar + side panels ----------

    fn activity_bar(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        let tabs = [
            (SideTab::Explorer, icons::FILES, "Explorer"),
            (SideTab::Git, icons::GIT_BRANCH, "Source Control"),
        ];
        for (tab, icon, tip) in tabs {
            let active = self.side_open && self.side_tab == tab;
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::click());
            let v = ui.visuals();
            let color = if active { v.strong_text_color() } else if resp.hovered() { v.text_color() } else { v.weak_text_color() };
            let accent = v.hyperlink_color;
            let hover_bg = v.widgets.hovered.weak_bg_fill;
            let painter = ui.painter();
            if resp.hovered() && !active {
                painter.rect_filled(rect.shrink2(egui::vec2(5.0, 3.0)), 6.0, hover_bg);
            }
            if active {
                painter.rect_filled(egui::Rect::from_min_size(rect.left_top(), egui::vec2(2.5, rect.height())), 0.0, accent);
            }
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, icon, FontId::new(21.0, lucide_family()), color);
            if resp.on_hover_text(tip).clicked() {
                if self.side_tab == tab {
                    self.side_open = !self.side_open;
                } else {
                    self.side_tab = tab;
                    self.side_open = true;
                }
            }
        }
    }

    fn explorer_panel(&mut self, ui: &mut Ui) {
        panel_header(ui, "EXPLORER", |ui| {
            let btn = egui::Button::new(icon_label(icons::REFRESH, "")).frame(false);
            ui.add_enabled(false, btn);
        });
        if let Some(root) = &self.explorer_root {
            ui.label(RichText::new(root.file_name().unwrap_or_default().to_string_lossy()).strong());
        }

        let mut to_load: Option<PathBuf> = None;
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if self.explorer_tree.is_empty() {
                ui.label(RichText::new("No folder open").weak());
                ui.label(RichText::new("File \u{2192} Open Folder\u{2026}").weak());
                return;
            }
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                draw_tree(ui, &self.explorer_tree, self.path.as_deref(), &mut to_load, 0);
            });
        });
        if let Some(p) = to_load {
            self.load(p.to_str().unwrap_or(""));
        }
    }

    fn git_panel(&mut self, ui: &mut Ui) {
        panel_header(ui, "SOURCE CONTROL", |ui| {
            let btn = egui::Button::new(icon_label(icons::REFRESH, "")).frame(false);
            ui.add_enabled(false, btn);
        });
        ui.add_space(48.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(icons::GIT_BRANCH).font(FontId::new(40.0, lucide_family())).weak());
            ui.add_space(10.0);
            ui.label(RichText::new("Nothing here yet").strong());
            ui.label(RichText::new("Git support is coming soon.").weak());
        });
    }

    fn blocks_panel(&mut self, ui: &mut Ui) {
        let mut counts: std::collections::HashMap<String, usize> = Default::default();
        let mut total = 0usize;
        for b in &self.blocks {
            if self.hide_c && b.group == "Comments" { continue; }
            *counts.entry(b.group.clone()).or_insert(0) += 1;
            total += 1;
        }
        let lang_groups = self.lang.groups();

        panel_header(ui, "CODE BLOCKS", |ui| {
            let shown = if self.cur == "All" {
                format!("All ({total})")
            } else {
                format!("{} ({})", self.cur, counts.get(&self.cur).copied().unwrap_or(0))
            };
            egui::ComboBox::new("group_filter", "")
                .selected_text(shown)
                .width(160.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.cur, "All".to_string(), format!("All ({total})"));
                    for g in lang_groups {
                        if let Some(&n) = counts.get(*g) {
                            ui.selectable_value(&mut self.cur, g.to_string(), format!("{g} ({n})"));
                        }
                    }
                    for g in ["Misc", "Errors"] {
                        if lang_groups.contains(&g) { continue; }
                        if let Some(&n) = counts.get(g) {
                            ui.selectable_value(&mut self.cur, g.to_string(), format!("{g} ({n})"));
                        }
                    }
                });
        });

        ui.add(
            TextEdit::singleline(&mut self.search)
                .id(egui::Id::new("block_search"))
                .hint_text("Search blocks…")
                .desired_width(f32::INFINITY),
        );
        ui.add_space(4.0);
        self.tree(ui);
    }

    // ---------- terminal (layout only for now) ----------

    fn toggle_terminal(&mut self, _ctx: &Context) {
        self.term_open = !self.term_open;
    }

    fn terminal_panel(&mut self, ui: &mut Ui) {
        panel_header(ui, "TERMINAL", |ui| {
            let close = egui::Button::new(icon_label(icons::X, "")).frame(false);
            if ui.add(close).on_hover_text("Close terminal (Ctrl+`)").clicked() {
                self.term_open = false;
            }
        });
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("Terminal \u{2014} coming soon").weak());
        });
    }

    fn tree(&mut self, ui: &mut Ui) {
        let q = self.search.to_lowercase();
        let visible: Vec<usize> = self.blocks.iter().enumerate()
        .filter(|(_, b)| {
            if self.hide_c && b.group == "Comments" { return false; }
            if self.cur != "All" && b.group != self.cur { return false; }
            if !q.is_empty() {
                let hay = format!("{} {} {}", b.group, b.title, b.sub).to_lowercase();
                if !hay.contains(&q) { return false; }
            }
            true
        })
        .map(|(i, _)| i)
        .collect();

        let mut by_group: std::collections::BTreeMap<String, Vec<usize>> = Default::default();
        for &i in &visible {
            by_group.entry(self.blocks[i].group.clone()).or_default().push(i);
        }

        let order: Vec<String> = {
            let mut v: Vec<String> = self.lang.groups().iter().map(|s| s.to_string()).collect();
            for g in by_group.keys() {
                if !v.contains(g) { v.push(g.clone()); }
            }
            v
        };

        ScrollArea::vertical().show(ui, |ui| {
            let grouped = self.cur == "All" && by_group.len() > 1;
            for g in &order {
                let Some(idxs) = by_group.get(g) else { continue };
                if grouped && idxs.len() > 1 {
                    let header = format!("{}  ({})", g, idxs.len());
                    let default_open = !q.is_empty() || self.expanded.contains(g);
                    let collapsing = egui::CollapsingHeader::new(header)
                    .default_open(default_open)
                    .id_source(g)
                    .show(ui, |ui| {
                        for &i in idxs {
                            self.block_row(ui, i, true);
                        }
                    });
                    if collapsing.header_response.clicked() {
                        if self.expanded.contains(g) { self.expanded.remove(g); }
                        else { self.expanded.insert(g.clone()); }
                    }
                } else {
                    for &i in idxs {
                        self.block_row(ui, i, false);
                    }
                }
            }
        });
    }

    fn block_row(&mut self, ui: &mut Ui, i: usize, child: bool) {
        let b = &self.blocks[i];
        let line_num = self.source[..b.range.start].matches('\n').count() + 1;
        let label = b.label(self.cur == "All" && !child);

        let mut text = RichText::new(label);
        if b.group == "Errors" { text = text.color(Color32::from_rgb(220, 90, 90)); }
        if b.group == "Disabled" || b.disabled { text = text.color(Color32::GRAY); }

        let mut clicked = false;
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let num_text = RichText::new(format!("{:>4}", line_num)).weak().monospace();
            if ui.add(egui::Label::new(num_text).selectable(false)).clicked() {
                clicked = true;
            }
            if ui.selectable_label(self.sel == Some(i), text).clicked() {
                clicked = true;
            }
        });

        if clicked {
            self.sel = Some(i);
            self.tab = 0;
        }
        ui.add_space(4.0);
    }

    /// Removes full-line comments. Returns the visible text plus the ORIGINAL
    /// 1-based line number of every remaining line (for the gutter).
    fn hide_comments_in_text(text: &str, lang: LanguageId) -> (String, Vec<usize>) {
        let Some(lc) = lang.line_comment() else {
            let n = text.lines().count().max(1);
            return (text.to_string(), (1..=n).collect());
        };
        let mut out = String::with_capacity(text.len());
        let mut nums = Vec::new();
        let mut only_blank_before = true;
        for (n, line) in text.split_inclusive('\n').enumerate() {
            let t = line.trim_start();
            let real = is_real_comment(t, lc, n, only_blank_before);
            if !t.trim().is_empty() { only_blank_before = false; }
            if real { continue; }
            out.push_str(line);
            nums.push(n + 1);
        }
        if nums.is_empty() { nums.push(1); }
        (out, nums)
    }

    fn editor_pane(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.horizontal(|ui| {
            if ui.selectable_label(self.tab == 0, "Block").clicked() { self.switch_tab(0); }
            if ui.selectable_label(self.tab == 1, "Raw").clicked() { self.switch_tab(1); }
            ui.separator();
            if let Some(p) = &self.path {
                ui.label(RichText::new(format!("{} — {}", p.display(), self.lang.name())).weak());
            } else {
                ui.label(RichText::new(format!("(unsaved) — {}", self.lang.name())).weak());
            }
        });
        ui.separator();

        let font_size = 13.0f32;
        if self.tab == 0 {
            let Some(i) = self.sel else {
                ui.centered_and_justified(|ui| { ui.label("Select a block"); });
                return;
            };
            let mut text = self.source[self.blocks[i].range.clone()].to_string();
            let row_height = ui.fonts(|f| f.row_height(&FontId::monospace(font_size)));

            let resp = {
                ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        draw_gutter(ui, text.lines().count().max(1), row_height, None);
                        ui.separator();

                        if text.len() < HL_MAX_BYTES {
                            let mut layouter = self.hl.layouter(self.lang, font_size);
                            let ed = TextEdit::multiline(&mut text)
                            .code_editor()
                            .font(FontId::monospace(font_size))
                            .desired_width(f32::INFINITY)
                            .desired_rows(30)
                            .layouter(&mut layouter);
                            ui.add(ed)
                        } else {
                            let ed = TextEdit::multiline(&mut text)
                            .code_editor()
                            .font(FontId::monospace(font_size))
                            .desired_width(f32::INFINITY)
                            .desired_rows(30);
                            ui.add(ed)
                        }
                    }).inner
                }).inner
            };

            if resp.changed() {
                self.apply_block_edit(i, text);
            }
        } else {
            let (mut text, line_nums) = if self.hide_c {
                let (t, n) = Self::hide_comments_in_text(&self.raw_buf, self.lang);
                (t, Some(n))
            } else {
                (self.raw_buf.clone(), None)
            };
            let row_height = ui.fonts(|f| f.row_height(&FontId::monospace(font_size)));

            let resp = {
                ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        draw_gutter(ui, text.lines().count().max(1), row_height, line_nums.as_deref());
                        ui.separator();

                        if text.len() < HL_MAX_BYTES {
                            let mut layouter = self.hl.layouter(self.lang, font_size);
                            let ed = TextEdit::multiline(&mut text)
                            .code_editor()
                            .font(FontId::monospace(font_size))
                            .desired_width(f32::INFINITY)
                            .desired_rows(30)
                            .interactive(!self.hide_c)
                            .layouter(&mut layouter);
                            ui.add(ed)
                        } else {
                            let ed = TextEdit::multiline(&mut text)
                            .code_editor()
                            .font(FontId::monospace(font_size))
                            .desired_width(f32::INFINITY)
                            .desired_rows(30)
                            .interactive(!self.hide_c);
                            ui.add(ed)
                        }
                    }).inner
                }).inner
            };

            if resp.changed() && !self.hide_c {
                self.raw_buf = text;
                self.raw_dirty_since = Some(Instant::now());
                self.dirty = true;
                ctx.request_repaint_after(Duration::from_secs(1));
            }
        }
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if let Some(msg) = &self.status_msg {
                ui.label(RichText::new(format!("{}{}", self.status_kind.prefix(), msg)).color(self.status_kind.color()));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("{} blocks", self.blocks.len())).weak());
                ui.separator();
                let cursor_info = if self.tab == 0 {
                    self.sel.map(|i| format!("block {}/{}", i + 1, self.blocks.len())).unwrap_or_default()
                } else { String::from("raw") };
                ui.label(RichText::new(cursor_info).weak());
            });
        });
    }

    fn apply_block_edit(&mut self, i: usize, new_text: String) {
        let old_start = self.blocks[i].range.start;
        self.source.replace_range(self.blocks[i].range.clone(), &new_text);
        self.reparse();
        self.sel = find_near(&self.blocks, old_start).or_else(|| self.blocks.first().map(|_| 0));
        self.dirty = true;
        self.block_dirty_since = Some(Instant::now());
    }

    fn confirm_close(&mut self) -> bool {
        if !self.dirty { return true; }
        let r = rfd::MessageDialog::new()
        .set_title("Unsaved changes")
        .set_description("Save before closing?")
        .set_buttons(rfd::MessageButtons::YesNoCancel)
        .show();
        match r {
            rfd::MessageDialogResult::Yes => self.save(),
            rfd::MessageDialogResult::No => true,
            _ => false,
        }
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        let (ctrl_s, ctrl_o, ctrl_n, ctrl_d, ctrl_e, ctrl_k) = ctx.input(|i| (
            i.modifiers.command && i.key_pressed(Key::S),
                                                                              i.modifiers.command && i.key_pressed(Key::O),
                                                                              i.modifiers.command && i.key_pressed(Key::N),
                                                                              i.modifiers.command && i.key_pressed(Key::D),
                                                                              i.modifiers.command && i.key_pressed(Key::E),
                                                                              i.modifiers.command && i.key_pressed(Key::K),
        ));
        let (ctrl_f, shift) = ctx.input(|i| (i.modifiers.command && i.key_pressed(Key::F), i.modifiers.shift));
        if ctrl_s { if shift { self.save_as(); } else { self.save(); } }
        if ctrl_f { ctx.memory_mut(|m| m.request_focus(egui::Id::new("block_search"))); }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Backtick)) { self.toggle_terminal(ctx); }
        if ctrl_o { self.open_dialog(); }
        if ctrl_n { self.new_block(); }
        if ctrl_d { if let Some(i) = self.sel { self.show_confirm_delete = Some(i); } }
        if ctrl_e { self.toggle_disabled(); }
        if ctrl_k && self.lang == LanguageId::Kdl {
            if let Some(i) = self.sel {
                if self.blocks[i].in_binds { self.start_capture(CaptureTarget::Recapture(i)); }
            }
        }
    }

    fn draw_help_windows(&mut self, ctx: &Context) {
        egui::Window::new("About Codelab")
            .open(&mut self.show_about)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(320.0);
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Codelab").size(26.0).strong());
                    ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).weak());
                    ui.add_space(8.0);
                    ui.label("A block-based code editor written in Rust.");
                    ui.label(RichText::new("Built with egui, tree-sitter and syntect.").weak());
                    ui.add_space(8.0);
                    ui.hyperlink_to("github.com/Tinnth7/codelab", REPO_URL);
                    ui.add_space(6.0);
                });
            });

        egui::Window::new("Keyboard Shortcuts")
            .open(&mut self.show_shortcuts)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Grid::new("shortcuts_grid").num_columns(2).spacing([24.0, 6.0]).show(ui, |ui| {
                    for (keys, what) in SHORTCUTS {
                        ui.label(RichText::new(*keys).monospace().strong());
                        ui.label(*what);
                        ui.end_row();
                    }
                });
            });
    }

    fn draw_capture_dialog(&mut self, ctx: &Context) {
        let Some(state) = self.capture.as_mut() else { return };

        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            if let egui::Event::Key { key, pressed, modifiers, .. } = ev {
                state.on_key(key, pressed, modifiers);
            }
        }

        let mut close = false;
        let mut accepted: Option<String> = None;
        if let Some(res) = &state.result {
            match res {
                Ok(combo) => { accepted = Some(combo.clone()); close = true; }
                Err(_) => { close = true; }
            }
        }

        let mut cancelled = false;
        egui::Window::new("Capture key")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.add_space(10.0);
            ui.label(RichText::new(&state.live).size(24.0).strong());
            ui.add_space(6.0);
            ui.label("Press the key combo. Esc alone cancels. Release all keys to accept.");
            ui.add_space(10.0);
            if ui.button("Cancel").clicked() { cancelled = true; }
        });

        if close || cancelled {
            self.capture = None;
            if let Some(combo) = accepted { self.finish_capture(combo); }
        }
    }
}

/// True only for real full-line comments. Shebangs and encoding cookies
/// look like `#` comments but aren't.
fn is_real_comment(trimmed: &str, lc: &str, line_no: usize, only_blank_before: bool) -> bool {
    if !trimmed.starts_with(lc) { return false; }
    if lc == "#" {
        if only_blank_before && trimmed.starts_with("#!") { return false; }
        if line_no < 2 && (trimmed.contains("coding:") || trimmed.contains("coding=")) { return false; }
    }
    true
}

/// `numbers`: original line number per displayed row (used when comments are hidden).
fn draw_gutter(ui: &mut Ui, count: usize, row_height: f32, numbers: Option<&[usize]>) {
    let max_label = numbers.and_then(|n| n.last().copied()).unwrap_or(count);
    let digits = max_label.to_string().len().max(2);
    let width = digits as f32 * 8.0 + 8.0;
    let weak = ui.visuals().weak_text_color();
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width, row_height * count as f32),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);

    let clip_rect = painter.clip_rect();
    let min_y = clip_rect.min.y - rect.min.y;
    let max_y = clip_rect.max.y - rect.min.y;

    let start_line = (min_y / row_height).floor().max(0.0) as usize;
    let end_line = (max_y / row_height).ceil().min(count as f32) as usize;

    for i in start_line..end_line {
        let label = numbers.and_then(|n| n.get(i).copied()).unwrap_or(i + 1);
        painter.text(
            rect.left_top() + egui::vec2(width - 4.0, row_height * i as f32),
            egui::Align2::RIGHT_TOP,
            label.to_string(),
            FontId::monospace(13.0),
            weak,
        );
    }
}

/// A node in the explorer tree. `children: Some(..)` = directory.
struct FileNode {
    path: PathBuf,
    name: String,
    children: Option<Vec<FileNode>>,
}

const EXPLORER_EXTS: &[&str] = &[
    "py", "cpp", "cc", "cxx", "c", "h", "hpp", "hh", "hxx", "js", "mjs", "jsx", "ts", "tsx",
    "rs", "kdl", "txt", "toml", "md", "json", "yaml", "yml",
];

/// Folders first, then files, both alphabetical. Empty folders are dropped.
fn scan_tree(dir: &Path) -> Vec<FileNode> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut dirs: Vec<FileNode> = Vec::new();
    let mut files: Vec<FileNode> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') { continue; }
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };

        if ft.is_dir() {
            if name == "target" || name == "node_modules" { continue; }
            let children = scan_tree(&path);
            if !children.is_empty() {
                dirs.push(FileNode { path, name, children: Some(children) });
            }
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if EXPLORER_EXTS.contains(&ext.as_str()) {
                files.push(FileNode { path, name, children: None });
            }
        }
    }

    dirs.sort_by_key(|n| n.name.to_lowercase());
    files.sort_by_key(|n| n.name.to_lowercase());
    dirs.extend(files);
    dirs
}

/// Short colored badge per file type (2 monospace cells wide so names line up).
fn file_icon(name: &str) -> (&'static str, Color32) {
    let ext = Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "rs" => ("R", Color32::from_rgb(222, 140, 90)),
        "py" => ("Py", Color32::from_rgb(90, 150, 210)),
        "c" => ("C", Color32::from_rgb(100, 140, 210)),
        "h" => ("H", Color32::from_rgb(160, 120, 210)),
        "cpp" | "cc" | "cxx" => ("C+", Color32::from_rgb(80, 130, 210)),
        "hpp" | "hh" | "hxx" => ("H+", Color32::from_rgb(160, 120, 210)),
        "js" | "mjs" | "jsx" => ("JS", Color32::from_rgb(235, 215, 80)),
        "ts" | "tsx" => ("TS", Color32::from_rgb(60, 130, 210)),
        "kdl" => ("K", Color32::from_rgb(190, 140, 230)),
        "toml" => ("T", Color32::from_rgb(160, 160, 160)),
        "md" => ("M", Color32::from_rgb(110, 175, 225)),
        "json" => ("{}", Color32::from_rgb(215, 185, 85)),
        "yaml" | "yml" => ("Y", Color32::from_rgb(210, 110, 110)),
        _ => ("\u{2022}", Color32::GRAY),
    }
}

fn draw_tree(ui: &mut Ui, nodes: &[FileNode], current: Option<&Path>, to_load: &mut Option<PathBuf>, depth: usize) {
    for node in nodes {
        match &node.children {
            Some(children) => {
                let has_current = current.map_or(false, |c| c.starts_with(&node.path));
                ui.push_id(&node.path, |ui| {
                    egui::CollapsingHeader::new(RichText::new(&node.name).strong())
                        .default_open(depth == 0 || has_current)
                        .show(ui, |ui| draw_tree(ui, children, current, to_load, depth + 1));
                });
            }
            None => {
                let selected = current == Some(node.path.as_path());
                let (glyph, color) = file_icon(&node.name);
                let mut job = egui::text::LayoutJob::default();
                job.append(
                    &format!("{:<2}", glyph),
                    0.0,
                    egui::TextFormat { font_id: FontId::monospace(12.0), color, ..Default::default() },
                );
                job.append(
                    &node.name,
                    6.0,
                    egui::TextFormat {
                        font_id: FontId::proportional(14.0),
                        color: ui.visuals().text_color(),
                        ..Default::default()
                    },
                );
                if ui.selectable_label(selected, job).clicked() {
                    *to_load = Some(node.path.clone());
                }
            }
        }
    }
}

impl eframe::App for Lab {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) {
            if !self.confirm_close() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }

        self.tick_timers(ctx);
        self.maybe_autosave(ctx);

        // Menu Bar
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.add(egui::Button::new("Open File\u{2026}").shortcut_text("Ctrl+O")).clicked() { self.open_dialog(); ui.close_menu(); }
                    if ui.button("Open Folder\u{2026}").clicked() { self.open_folder_dialog(); ui.close_menu(); }
                    ui.separator();
                    if ui.add(egui::Button::new("Save").shortcut_text("Ctrl+S")).clicked() { self.save(); ui.close_menu(); }
                    if ui.add(egui::Button::new("Save As\u{2026}").shortcut_text("Ctrl+Shift+S")).clicked() { self.save_as(); ui.close_menu(); }
                    ui.separator();
                    if ui.button("Exit").clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.add(egui::Button::new("New Block").shortcut_text("Ctrl+N")).clicked() { self.new_block(); ui.close_menu(); }
                    if ui.add(egui::Button::new("Delete Block").shortcut_text("Ctrl+D")).clicked() { if let Some(i) = self.sel { self.show_confirm_delete = Some(i); } ui.close_menu(); }
                    if ui.add(egui::Button::new("Toggle Disable").shortcut_text("Ctrl+E")).clicked() { self.toggle_disabled(); ui.close_menu(); }
                    ui.separator();
                    if ui.add(egui::Button::new("Find Blocks").shortcut_text("Ctrl+F")).clicked() {
                        ctx.memory_mut(|m| m.request_focus(egui::Id::new("block_search")));
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.checkbox(&mut self.dark, "Dark Mode").changed() {
                        if self.dark { ctx.set_visuals(egui::Visuals::dark()); }
                        else { ctx.set_visuals(egui::Visuals::light()); }
                    }
                    ui.checkbox(&mut self.hide_c, "Hide Comments");
                    ui.checkbox(&mut self.autosave, "Autosave (every 5 min)");
                    ui.separator();
                    ui.checkbox(&mut self.side_open, "Side Bar");
                    if ui.add(egui::Button::new("Terminal").shortcut_text("Ctrl+`")).clicked() { self.toggle_terminal(ctx); ui.close_menu(); }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Keyboard Shortcuts").clicked() { self.show_shortcuts = true; ui.close_menu(); }
                    ui.separator();
                    if ui.button("GitHub Repository").clicked() { ctx.open_url(egui::OpenUrl::new_tab(REPO_URL)); ui.close_menu(); }
                    if ui.button("Report an Issue").clicked() { ctx.open_url(egui::OpenUrl::new_tab(format!("{REPO_URL}/issues"))); ui.close_menu(); }
                    ui.separator();
                    if ui.button("About Codelab").clicked() { self.show_about = true; ui.close_menu(); }
                });
            });
        });

        // Toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| self.toolbar(ui));
        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| self.status_bar(ui));

        // Layout: activity bar | side tab content | code blocks | editor (+ terminal below it)
        egui::SidePanel::left("activity")
        .exact_width(44.0)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::none().fill(ctx.style().visuals.extreme_bg_color))
        .show(ctx, |ui| self.activity_bar(ui));

        if self.side_open {
            egui::SidePanel::left("side")
            .resizable(true)
            .default_width(240.0)
            .min_width(160.0)
            .show(ctx, |ui| match self.side_tab {
                SideTab::Explorer => self.explorer_panel(ui),
                SideTab::Git => self.git_panel(ui),
            });
        }

        egui::SidePanel::left("blocks")
        .resizable(true)
        .default_width(400.0)
        .min_width(260.0)
        .show(ctx, |ui| self.blocks_panel(ui));

        if self.term_open {
            egui::TopBottomPanel::bottom("terminal")
            .resizable(true)
            .default_height(200.0)
            .min_height(100.0)
            .show(ctx, |ui| self.terminal_panel(ui));
        }

        egui::CentralPanel::default().show(ctx, |ui| self.editor_pane(ui, ctx));

        self.draw_capture_dialog(ctx);
        self.draw_help_windows(ctx);
        self.handle_shortcuts(ctx);

        if let Some(i) = self.show_confirm_delete.take() {
            let r = rfd::MessageDialog::new()
            .set_title("Delete block")
            .set_description(&format!("Delete block \"{}\"?", self.blocks.get(i).map(|b| b.title.as_str()).unwrap_or("")))
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
            if r == rfd::MessageDialogResult::Yes {
                self.sel = Some(i);
                self.delete_selected();
            }
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let state = PersistedState {
            dark: self.dark,
            hide_c: self.hide_c,
            autosave: self.autosave,
            backup_dir: self.backup_dir.clone(),
        };
        if let Ok(s) = serde_json::to_string(&state) {
            storage.set_string("codelab_state", s);
        }
    }
}

fn write_snapshot(path: &Path, content: &[u8], dir: &Path) -> anyhow::Result<Option<PathBuf>> {
    fs::create_dir_all(dir)?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    let mut existing: Vec<PathBuf> = fs::read_dir(dir)?
    .filter_map(|e| e.ok().map(|x| x.path()))
    .filter(|p| {
        let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        fname.starts_with(stem) && p.extension().and_then(|s| s.to_str()) == Some(ext)
    })
    .collect();
    existing.sort();

    if let Some(last) = existing.last() {
        if let Ok(bytes) = fs::read(last) {
            if bytes == content { return Ok(Some(last.clone())); }
        }
    }

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let mut dst = dir.join(format!("{stem}-{stamp}.{ext}"));
    let mut n = 0;
    while dst.exists() {
        n += 1;
        dst = dir.join(format!("{stem}-{stamp}-{n}.{ext}"));
    }
    fs::write(&dst, content)?;
    existing.push(dst.clone());
    existing.sort();
    let to_remove = existing.len().saturating_sub(50);
    for f in existing.iter().take(to_remove) {
        let _ = fs::remove_file(f);
    }
    Ok(Some(dst))
}
