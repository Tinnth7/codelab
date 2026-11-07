use std::ops::Range;

#[derive(Clone, Debug)]
pub struct Block {
    /// Byte range into `Lab::source`. Never modified except by re-split.
    pub range: Range<usize>,
    pub group: String,
    pub title: String,
    pub sub: String,
    pub disabled: bool,
    /// True if this block is an individual bind inside a `binds { ... }` wrapper.
    pub in_binds: bool,
}

#[derive(Clone)]
pub struct Classified {
    pub group: &'static str,
    pub title: String,
    pub sub: String,
}

impl Block {
    pub fn label(&self, show_group: bool) -> String {
        let mut s = self.title.clone();
        if !self.sub.is_empty() {
            s.push_str("    ");
            s.push_str(&self.sub);
        }
        if self.disabled {
            s.push_str("    (disabled)");
        }
        if show_group {
            s.push_str(&format!("    [{}]", self.group));
        }
        s
    }
}

/// Find the block containing `offset`, or the nearest one.
pub fn find_near(blocks: &[Block], offset: usize) -> Option<usize> {
    blocks
    .iter()
    .position(|b| b.range.contains(&offset))
    .or_else(|| {
        blocks
        .iter()
        .enumerate()
        .min_by_key(|(_, b)| b.range.start.abs_diff(offset))
        .map(|(i, _)| i)
    })
}

pub fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("").trim()
}
