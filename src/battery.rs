//! The last battery row actually emitted by a program, independent of its cursor.
//! Never fetch quota or infer usage. The shell prompt marks the end of its lifetime.
use alacritty_terminal::{
    event::EventListener,
    grid::Dimensions,
    index::{Column, Line},
    term::{
        cell::{Cell, Flags},
        color::Colors,
        Term,
    },
};

pub struct Row {
    pub line: i32,
    pub cells: Vec<Cell>,
    pub colors: Colors,
    pub text: String,
}

impl Row {
    pub fn find<T: EventListener>(term: &Term<T>) -> Option<Self> {
        (0..term.screen_lines() as i32).rev().find_map(|line| {
            // Most rows are ordinary output. Check a small label prefix before
            // copying cells from the committed synchronized frame.
            let prefix: String = (0..term.columns().min(32))
                .map(|column| term.grid()[Line(line)][Column(column)].c)
                .collect();
            let prefix = prefix.trim_start().to_ascii_lowercase();
            if ["ai battery", "ai-battery", "codex", "claude"]
                .iter()
                .any(|name| prefix.starts_with(name))
            {
                Self::capture(term, line)
            } else {
                None
            }
        })
    }

    pub fn capture<T: EventListener>(term: &Term<T>, line: i32) -> Option<Self> {
        if !(0..term.screen_lines() as i32).contains(&line) {
            return None;
        }
        let cells: Vec<_> = (0..term.columns().min(512))
            .map(|column| term.grid()[Line(line)][Column(column)].clone())
            .collect();
        let text: String = cells
            .iter()
            .filter(|cell| {
                !cell.flags.intersects(
                    Flags::HIDDEN | Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER,
                )
            })
            .flat_map(|cell| {
                std::iter::once(cell.c).chain(cell.zerowidth().into_iter().flatten().copied())
            })
            .collect();
        if !crate::mini::battery(text.trim()) {
            return None;
        }
        let end = cells
            .iter()
            .rposition(|cell| cell.c != ' ' || cell.zerowidth().is_some())
            .map_or(0, |index| index + 1);
        Some(Self {
            line,
            cells: cells[..end].to_vec(),
            colors: *term.colors(),
            text: text.trim().to_owned(),
        })
    }
}
