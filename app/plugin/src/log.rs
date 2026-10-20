//! A plugin's log lines, kept with its call as values.

use suzatary_core::plugin::Said;

/// The most lines kept per call.
pub(crate) const MAX_LINES: usize = 100;

/// How much a line matters, as the plugin said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Detail.
    Debug,
    /// Progress.
    Info,
    /// Something the operator should read.
    Warn,
}

/// One line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Its level.
    pub level: Level,
    /// Its text.
    pub text: Said,
}

/// The lines one call wrote: the first `MAX_LINES`, and how many more were dropped. The
/// binary decides which are worth a line of its own; a plugin's text reaches nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Log {
    /// The lines kept, in order.
    pub lines: Vec<Line>,
    /// How many were dropped past the cap.
    pub dropped: usize,
}

impl Log {
    pub(crate) fn push(&mut self, level: Level, text: Said) {
        if self.lines.len() < MAX_LINES {
            self.lines.push(Line { level, text });
        } else {
            self.dropped += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_keeps_a_bounded_number_of_lines_and_counts_the_rest() {
        let mut log = Log::default();
        for _ in 0..MAX_LINES + 7 {
            log.push(Level::Info, Said::clip("a line"));
        }
        assert_eq!(log.lines.len(), MAX_LINES);
        assert_eq!(log.dropped, 7);
    }
}
