//! EDT-009: the find/replace model shared by the editor, the find bar and the tests.
//!
//! Everything here is pure: it reads the editor's line vector, never touches egui and never
//! edits a buffer. The editor owns one [`FindSession`] per document; the window draws the bar
//! over the fields and calls the editor's action methods, so the same search rules are exercised
//! by the crate tests and by the shipped window.
//!
//! Matches are line-local: a match has one row and a half-open char-column range on that row.
//! That is what keeps `^` and `$` line anchors, makes every replacement stay inside the line it
//! matched, and lets the painter highlight a match with a single rectangle. Literal matching is
//! ASCII case-folded when the case-sensitive toggle is off; regular expressions come from
//! [`crate::regex_lite`], whose invalid patterns are values ([`FindSession::error`]), never
//! panics.

use std::collections::BTreeMap;

use crate::regex_lite;

/// The three toggles of the find bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FindOptions {
    /// Match case exactly. Off by default: a plain find is case-insensitive.
    pub case_sensitive: bool,
    /// Require the match to start and end on an identifier boundary.
    pub whole_word: bool,
    /// Interpret the query as a regular expression.
    pub regex: bool,
}

impl FindOptions {
    /// A short stable description for the pane header and the status line.
    pub fn tags(&self) -> String {
        let mut tags = Vec::new();
        if self.case_sensitive {
            tags.push("case-sensitive");
        }
        if self.whole_word {
            tags.push("whole-word");
        }
        if self.regex {
            tags.push("regex");
        }
        if tags.is_empty() {
            "default".to_string()
        } else {
            tags.join(", ")
        }
    }
}

/// One match: a row and a half-open range of char columns on that row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchSpan {
    /// 0-based buffer row.
    pub row: usize,
    /// 0-based first matched char column.
    pub start: usize,
    /// 0-based char column one past the last matched char.
    pub end: usize,
}

impl MatchSpan {
    /// A stable one-based `row:start-end` description for logs and tests.
    #[cfg_attr(not(test), allow(dead_code))] // the engine tests read it
    pub fn describe(&self) -> String {
        format!("{}:{}-{}", self.row + 1, self.start + 1, self.end)
    }
}

/// True at the characters a whole-word boundary treats as word characters.
fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// The line-local search run by the find bar and by `replace_all`.
///
/// The query being empty is not an error: it means "nothing to find", which the bar paints as
/// its empty state. An invalid regex is an error and clears the matches, so the bar can show the
/// reason while the buffer stays untouched.
pub fn find_matches(
    lines: &[String],
    query: &str,
    options: &FindOptions,
) -> Result<Vec<MatchSpan>, String> {
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let mut spans = Vec::new();
    if options.regex {
        let regex = regex_lite::compile(query)?;
        for (row, line) in lines.iter().enumerate() {
            let chars: Vec<char> = line.chars().collect();
            let found = regex.find_all(&chars, options.case_sensitive)?;
            for (start, end) in found {
                if options.whole_word && !is_whole_word(&chars, start, end) {
                    continue;
                }
                spans.push(MatchSpan { row, start, end });
            }
        }
        return Ok(spans);
    }
    let needle: Vec<char> = query.chars().collect();
    for (row, line) in lines.iter().enumerate() {
        let haystack: Vec<char> = line.chars().collect();
        let mut at = 0usize;
        while at + needle.len() <= haystack.len() {
            let matched = haystack[at..at + needle.len()]
                .iter()
                .zip(needle.iter())
                .all(|(left, right)| {
                    if options.case_sensitive {
                        left == right
                    } else {
                        left.to_ascii_lowercase() == right.to_ascii_lowercase()
                    }
                });
            if matched && (!options.whole_word || is_whole_word(&haystack, at, at + needle.len()))
            {
                spans.push(MatchSpan {
                    row,
                    start: at,
                    end: at + needle.len(),
                });
                at += needle.len();
            } else {
                at += 1;
            }
        }
    }
    Ok(spans)
}

fn is_whole_word(chars: &[char], start: usize, end: usize) -> bool {
    let before = start.checked_sub(1).and_then(|index| chars.get(index));
    let after = chars.get(end);
    !before.map(|ch| is_word_char(*ch)).unwrap_or(false)
        && !after.map(|ch| is_word_char(*ch)).unwrap_or(false)
}

/// The find bar's live state: the query, the toggles, the matches and the highlighted match.
///
/// The matches are cached and recomputed only when the query, the toggles or the buffer changed
/// ([`FindSession::refresh`] is passed the current lines). That keeps a 5 000-line document from
/// being rescanned on every paint pass, which matters for EDT-016's typing budget.
#[derive(Clone, Debug, Default)]
pub struct FindSession {
    /// True while the bar is shown.
    pub open: bool,
    /// The find field.
    pub query: String,
    /// The replace field.
    pub replacement: String,
    /// The three toggles.
    pub options: FindOptions,
    matches: Vec<MatchSpan>,
    /// Index of the highlighted match; only meaningful when `matches` is non-empty.
    selected: usize,
    /// The caret when the search started, used to choose the first match a fresh query lands on.
    anchor: (usize, usize),
    /// Set when the query, a toggle or the buffer changed and the matches must be recomputed.
    dirty: bool,
    /// Set by a query/toggle change: the next refresh selects from the anchor again instead of
    /// keeping the highlight index. A buffer edit keeps the highlight when it is still valid.
    reset_selection: bool,
    /// The invalid-regex reason, or `None` when the pattern is usable.
    error: Option<String>,
}

impl FindSession {
    /// Opens the bar at the caret and marks the matches stale.
    pub fn begin(&mut self, caret: (usize, usize)) {
        self.open = true;
        self.anchor = caret;
        self.selected = 0;
        self.dirty = true;
        self.reset_selection = true;
    }

    /// Closes the bar and drops every match so no highlight survives the close.
    pub fn close(&mut self) {
        self.open = false;
        self.matches.clear();
        self.selected = 0;
        self.error = None;
        self.dirty = true;
        self.reset_selection = true;
    }

    /// True while the bar is shown.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Replaces the query and marks the matches stale.
    #[cfg_attr(not(test), allow(dead_code))] // the tests drive the query through this seam
    pub fn set_query(&mut self, query: &str) {
        if self.query == query {
            return;
        }
        self.query = query.to_string();
        self.dirty = true;
        self.reset_selection = true;
    }

    /// Replaces the replacement text.
    #[cfg_attr(not(test), allow(dead_code))] // the bar edits the field; tests use the seam
    pub fn set_replacement(&mut self, replacement: &str) {
        self.replacement = replacement.to_string();
    }

    /// Marks a query edited directly by the find field as stale.
    ///
    /// The egui text widget needs `&mut String`, so it edits `query` in place; this records the
    /// same consequence as [`FindSession::set_query`] (recompute and re-anchor the highlight).
    pub fn query_changed(&mut self) {
        self.dirty = true;
        self.reset_selection = true;
    }

    /// Sets one toggle and marks the matches stale when it actually changed.
    pub fn set_option(&mut self, case_sensitive: bool, whole_word: bool, regex: bool) {
        let next = FindOptions {
            case_sensitive,
            whole_word,
            regex,
        };
        if next != self.options {
            self.options = next;
            self.dirty = true;
            self.reset_selection = true;
        }
    }

    /// Marks the cached matches stale (a buffer edit, an undo or a redo).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Re-anchors a later refresh at a buffer position (after a replacement).
    pub fn reselect_from(&mut self, caret: (usize, usize)) {
        self.anchor = caret;
        self.dirty = true;
        self.reset_selection = true;
    }

    /// Recomputes the matches when stale and keeps the highlight inside the new list.
    ///
    /// A fresh query (still `dirty` with no matches computed for it) selects the first match at
    /// or after the caret, wrapping to the first match in the document. A `refresh` that finds
    /// the same match list keeps the current highlight.
    pub fn refresh(&mut self, lines: &[String], caret: (usize, usize)) {
        if !self.open || !self.dirty {
            return;
        }
        self.dirty = false;
        match find_matches(lines, &self.query, &self.options) {
            Ok(matches) => {
                self.error = None;
                self.matches = matches;
                if self.matches.is_empty() {
                    self.selected = 0;
                } else if self.reset_selection {
                    self.selected = self
                        .first_at_or_after(self.anchor)
                        .or_else(|| self.first_at_or_after(caret))
                        .unwrap_or(0);
                } else if self.selected >= self.matches.len() {
                    self.selected = self.matches.len() - 1;
                }
                self.reset_selection = false;
            }
            Err(reason) => {
                self.error = Some(reason);
                self.matches.clear();
                self.selected = 0;
                self.reset_selection = false;
            }
        }
    }

    /// The first match that starts at or after `position`, or one that contains it.
    ///
    /// A caret inside a word that the query matched selects that word rather than jumping past
    /// it, which is what a find bar does when it opens with the caret in the text.
    fn first_at_or_after(&self, position: (usize, usize)) -> Option<usize> {
        self.matches.iter().position(|span| {
            (span.row, span.start) >= position
                || (span.row == position.0 && span.start < position.1 && position.1 < span.end)
        })
    }

    /// The invalid-pattern reason, when the last refresh rejected the query.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Every match for the current query, in document order.
    pub fn matches(&self) -> &[MatchSpan] {
        &self.matches
    }

    /// The highlighted match's index into [`FindSession::matches`].
    #[cfg_attr(not(test), allow(dead_code))] // read by the header tests and the captures
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// The highlighted match.
    pub fn selected(&self) -> Option<MatchSpan> {
        self.matches.get(self.selected).copied()
    }

    /// Moves the highlight to the next match, wrapping at the end.
    pub fn select_next(&mut self) -> Option<MatchSpan> {
        if self.matches.is_empty() {
            return None;
        }
        self.selected = (self.selected + 1) % self.matches.len();
        self.selected()
    }

    /// Moves the highlight to the previous match, wrapping at the start.
    pub fn select_previous(&mut self) -> Option<MatchSpan> {
        if self.matches.is_empty() {
            return None;
        }
        self.selected = if self.selected == 0 {
            self.matches.len() - 1
        } else {
            self.selected - 1
        };
        self.selected()
    }

    /// Points the highlight at a specific match, if it is still in range.
    #[cfg_attr(not(test), allow(dead_code))] // part of the session surface; tests pin it
    pub fn select(&mut self, index: usize) -> Option<MatchSpan> {
        if index >= self.matches.len() {
            return None;
        }
        self.selected = index;
        self.selected()
    }

    /// The bar's match-count text: `3 of 12`, `No matches`, or the empty state.
    ///
    /// The invalid-pattern reason is painted separately by the bar (in the error colour); this
    /// returns `None` for it so the count cannot present a failed search as "No matches".
    pub fn count_text(&self) -> Option<String> {
        if self.error.is_some() {
            return None;
        }
        if self.query.is_empty() {
            return Some("Type to find".to_string());
        }
        if self.matches.is_empty() {
            return Some("No matches".to_string());
        }
        Some(format!("{} of {}", self.selected + 1, self.matches.len()))
    }

    /// The one-line state the pane header publishes, so a UIA capture can assert the count.
    pub fn summary(&self) -> Option<String> {
        if !self.open {
            return None;
        }
        if let Some(reason) = &self.error {
            return Some(format!("Find: invalid regex - {reason}"));
        }
        match self.count_text() {
            Some(count) => Some(format!(
                "Find: '{}' [{}] - {count}",
                self.query,
                self.options.tags()
            )),
            None => None,
        }
    }

    /// Groups the matches by row, in ascending row order, for `replace_all`.
    pub fn by_row(&self) -> BTreeMap<usize, Vec<MatchSpan>> {
        let mut rows: BTreeMap<usize, Vec<MatchSpan>> = BTreeMap::new();
        for span in &self.matches {
            rows.entry(span.row).or_default().push(*span);
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.split('\n').map(str::to_string).collect()
    }

    fn spans(text: &str, query: &str, options: FindOptions) -> Vec<(usize, usize, usize)> {
        find_matches(&lines(text), query, &options)
            .expect("a usable pattern")
            .into_iter()
            .map(|span| (span.row, span.start, span.end))
            .collect()
    }

    #[test]
    fn literal_search_finds_each_non_overlapping_match_once() {
        let found = find_matches(
            &lines("alpha beta alpha\nGamma alpha"),
            "alpha",
            &FindOptions::default(),
        )
        .expect("a usable pattern");
        assert_eq!(
            found
                .iter()
                .map(|span| (span.row, span.start, span.end))
                .collect::<Vec<_>>(),
            vec![(0, 0, 5), (0, 11, 16), (1, 6, 11)],
            "three matches, exact rows and columns"
        );
        assert_eq!(found[0].describe(), "1:1-5");
        assert_eq!(found[2].describe(), "2:7-11");
        assert_eq!(
            spans("aaaa", "aa", FindOptions::default()),
            vec![(0, 0, 2), (0, 2, 4)],
            "matches do not overlap"
        );
    }

    #[test]
    fn case_sensitivity_follows_the_toggle() {
        let text = "Alpha alpha ALPHA";
        assert_eq!(
            spans(text, "alpha", FindOptions::default()).len(),
            3,
            "the default is case-insensitive"
        );
        assert_eq!(
            spans(
                text,
                "alpha",
                FindOptions {
                    case_sensitive: true,
                    ..FindOptions::default()
                }
            ),
            vec![(0, 6, 11)],
            "case-sensitive matches only the exact lowercase word"
        );
    }

    #[test]
    fn whole_word_excludes_identifier_prefixes_and_suffixes() {
        let text = "cat category cat_1 cat2 cat";
        let options = FindOptions {
            whole_word: true,
            ..FindOptions::default()
        };
        assert_eq!(
            spans(text, "cat", options),
            vec![(0, 0, 3), (0, 24, 27)],
            "only the two standalone identifiers are whole-word matches"
        );
    }

    #[test]
    fn a_regex_pattern_reports_the_engine_error_instead_of_a_match_list() {
        let options = FindOptions {
            regex: true,
            ..FindOptions::default()
        };
        for pattern in ["(", "[", "*", "a{2,1}", "\\"] {
            let error = find_matches(&lines("abc"), pattern, &options)
                .expect_err("an invalid pattern is an error");
            assert!(error.contains("Invalid regular expression"), "{error}");
        }
        assert_eq!(
            spans("a1 b22 c333", "\\d+", options),
            vec![(0, 1, 2), (0, 4, 6), (0, 8, 11)],
            "a valid regex still finds every run"
        );
    }

    #[test]
    fn whole_word_applies_to_regex_matches_too() {
        let options = FindOptions {
            regex: true,
            whole_word: true,
            case_sensitive: true,
        };
        assert_eq!(
            spans("ab 12 abc 12", "[a-z]+", options),
            vec![(0, 0, 2), (0, 6, 9)],
            "the letter runs are identifier-bounded on both sides"
        );
    }

    #[test]
    fn a_session_selects_the_caret_match_and_wraps_in_both_directions() {
        let text = "one two one two one";
        let mut session = FindSession::default();
        session.begin((0, 9)); // on the second "two"
        session.set_query("one");
        session.refresh(&lines(text), (0, 9));
        assert_eq!(session.count_text().as_deref(), Some("2 of 3"));
        assert_eq!(session.selected(), Some(MatchSpan { row: 0, start: 8, end: 11 }));

        assert_eq!(
            session.select_next(),
            Some(MatchSpan {
                row: 0,
                start: 16,
                end: 19
            })
        );
        assert_eq!(
            session.select_next(),
            Some(MatchSpan {
                row: 0,
                start: 0,
                end: 3
            }),
            "next wraps to the first match"
        );
        assert_eq!(
            session.select_previous(),
            Some(MatchSpan {
                row: 0,
                start: 16,
                end: 19
            }),
            "previous wraps back to the last match"
        );
        assert_eq!(session.count_text().as_deref(), Some("3 of 3"));
    }

    #[test]
    fn an_invalid_pattern_has_no_count_and_keeps_the_reason() {
        let mut session = FindSession::default();
        session.begin((0, 0));
        session.set_query("(");
        session.set_option(false, false, true);
        session.refresh(&lines("abc"), (0, 0));
        assert!(session.matches().is_empty());
        assert!(session.error().is_some());
        assert_eq!(session.count_text(), None);
        assert!(
            session.summary().expect("open").starts_with("Find: invalid regex"),
            "the summary keeps the rejection reason"
        );
    }

    #[test]
    fn closing_the_session_drops_every_match() {
        let mut session = FindSession::default();
        session.begin((0, 0));
        session.set_query("a");
        session.refresh(&lines("a a"), (0, 0));
        assert_eq!(session.matches().len(), 2);
        session.close();
        assert!(session.matches().is_empty());
        assert_eq!(session.summary(), None);
        assert!(!session.is_open());
    }
}
