//! Budget the fixed grid storage by bytes as well as the user's history lines.
use alacritty_terminal::{
    event::EventListener,
    grid::Dimensions,
    term::{cell::Cell, Config, Osc52, Term},
};

pub(crate) const HISTORY_BYTES: usize = 32 * 1024 * 1024;

pub(crate) fn history_lines(requested: usize, columns: usize) -> usize {
    requested.min(HISTORY_BYTES / std::mem::size_of::<Cell>() / columns.max(1))
}

pub(crate) fn config(requested: usize, columns: usize) -> Config {
    Config {
        scrolling_history: history_lines(requested, columns),
        osc52: Osc52::Disabled,
        ..Config::default()
    }
}

pub(crate) fn before_resize<T: EventListener>(
    term: &mut Term<T>,
    requested: usize,
    columns: usize,
) {
    // Trim the old narrow history BEFORE widening it. Trimming afterwards can
    // allocate the entire oversized grid during reflow.
    term.set_options(config(requested, columns.max(term.columns())));
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::{event::Event, grid::Dimensions, vte::ansi::Processor};
    #[derive(Clone, Copy)]
    struct Listener;
    impl EventListener for Listener {
        fn send_event(&self, _: Event) {}
    }

    #[test]
    fn combining_floods_are_bounded_across_parser_chunks_and_preserve_progress() {
        let mut term = Term::new(
            config(50_000, 80),
            &crate::terminal::GridSize { cols: 80, rows: 4 },
            Listener,
        );
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, "界".as_bytes());
        for _ in 0..100_000 {
            parser.advance(&mut term, "\u{301}".as_bytes());
        }
        let cell =
            &term.grid()[alacritty_terminal::index::Line(0)][alacritty_terminal::index::Column(0)];
        assert_eq!(cell.zerowidth().unwrap().len(), 64);
        let saved = cell.clone();
        parser.advance(&mut term, "\u{302}".as_bytes());
        assert!(std::sync::Arc::ptr_eq(
            saved.extra.as_ref().unwrap(),
            term.grid()[alacritty_terminal::index::Line(0)][alacritty_terminal::index::Column(0)]
                .extra
                .as_ref()
                .unwrap()
        ));
        parser.advance(&mut term, b"NEXT\r\nOK");
        let text: String = term
            .grid()
            .display_iter()
            .map(|indexed| indexed.cell.c)
            .collect();
        assert!(text.contains("NEXT"));
        assert!(text.contains("OK"));
        assert_eq!(saved.zerowidth().unwrap().len(), 64);
    }

    #[test]
    fn wide_resize_trims_history_before_reflow_and_preserves_recent_output() {
        let mut term = Term::new(
            config(50_000, 80),
            &crate::terminal::GridSize { cols: 80, rows: 4 },
            Listener,
        );
        let mut parser: Processor = Processor::new();
        for _ in 0..history_lines(50_000, 80) + 20 {
            parser.advance(&mut term, b"record\r\n");
        }
        parser.advance(&mut term, b"LATEST");
        assert!(term.grid().history_size() > history_lines(50_000, 1000));
        before_resize(&mut term, 50_000, 1000);
        assert!(term.grid().history_size() <= history_lines(50_000, 1000));
        term.resize(crate::terminal::GridSize {
            cols: 1000,
            rows: 4,
        });
        term.set_options(config(50_000, 1000));
        assert!(term.grid().history_size() <= history_lines(50_000, 1000));
        assert_eq!(term.columns(), 1000);
        let recent: String = term
            .grid()
            .display_iter()
            .map(|indexed| indexed.cell.c)
            .collect();
        assert!(recent.contains("LATEST"));
        assert!(
            history_lines(usize::MAX, 1000) * 1000 * std::mem::size_of::<Cell>() <= HISTORY_BYTES
        );
    }
}
