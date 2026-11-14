//! A small, bounded regular-expression engine for EDT-009's find bar.
//!
//! The desktop crate does not depend on the `regex` crate: `rust/lovelace-desktop/**` is the only
//! path this round may change, and adding a dependency would edit the shared workspace lockfile.
//! This module is therefore a self-contained engine for the subset a find bar needs:
//!
//! - literals, `.`, `^`, `$`;
//! - character classes `[abc]`, `[a-z]`, `[^...]`, and the shorthands `\d \w \s` (and `\D \W
//!   \S`), plus the escapes `\\ \. \n \t \r`;
//! - groups `( )` and `(?: )`, alternation `|`, and the quantifiers `* + ? {m} {m,} {m,n}`;
//! - case-insensitive matching at the request of the caller (ASCII folding, the identifier
//!   alphabet of the language).
//!
//! Safety is a hard requirement of the row: an invalid pattern must be a typed [`Err`], never a
//! panic, and a pathological pattern must not be able to hang the window. Two bounds provide
//! that:
//!
//! - [`MAX_INSTRUCTIONS`] caps the compiled program, and the parser caps nesting depth;
//! - [`STEP_BUDGET`] caps the backtracking machine's steps per attempted match, so nested
//!   quantifiers with an empty body can only report "too complex", never loop forever.
//!
//! Matching is leftmost-first and greedy. Matching is per line: the caller (the find bar) runs
//! the engine over one line at a time, which is what makes `^` and `$` line anchors and keeps
//! every replacement inside the line it matched.

/// Maximum number of characters in one pattern.
pub const MAX_PATTERN_CHARS: usize = 4096;
/// Maximum number of compiled instructions.
pub const MAX_INSTRUCTIONS: usize = 4096;
/// Maximum nesting of groups and classes during parsing.
const MAX_NESTING: usize = 64;
/// Maximum backtracking steps for one attempted match at one start position.
pub const STEP_BUDGET: usize = 200_000;
/// Maximum repetition count accepted in `{m,n}`.
const MAX_REPEAT: usize = 512;

/// One character-class member.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ClassItem {
    /// An inclusive range.
    Range(char, char),
    /// `\d` (`negated` false) or `\D` (true).
    Digit(bool),
    /// `\w` or `\W`.
    Word(bool),
    /// `\s` or `\S`.
    Space(bool),
    /// One literal character.
    Literal(char),
}

/// A character class.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Class {
    negated: bool,
    items: Vec<ClassItem>,
}

impl Class {
    fn matches(&self, ch: char, case_sensitive: bool) -> bool {
        let hit = self.items.iter().any(|item| match item {
            ClassItem::Range(start, end) => {
                if case_sensitive {
                    *start <= ch && ch <= *end
                } else {
                    let lower = ch.to_ascii_lowercase();
                    let upper = ch.to_ascii_uppercase();
                    (*start <= ch && ch <= *end)
                        || (*start <= lower && lower <= *end)
                        || (*start <= upper && upper <= *end)
                }
            }
            ClassItem::Digit(negated) => ch.is_ascii_digit() != *negated,
            ClassItem::Word(negated) => (ch.is_ascii_alphanumeric() || ch == '_') != *negated,
            ClassItem::Space(negated) => ch.is_whitespace() != *negated,
            ClassItem::Literal(literal) => {
                if case_sensitive {
                    ch == *literal
                } else {
                    ch.to_ascii_lowercase() == literal.to_ascii_lowercase()
                }
            }
        });
        hit != self.negated
    }
}

/// The parsed pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Empty,
    Char(char),
    Any,
    Class(usize),
    Start,
    End,
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    Repeat {
        node: Box<Node>,
        min: usize,
        max: Option<usize>,
    },
}

/// One instruction of the backtracking program.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Inst {
    Char(char),
    Any,
    Class(usize),
    Start,
    End,
    Split(usize, usize),
    Jump(usize),
    Match,
}

/// A compiled pattern.
#[derive(Clone, Debug)]
pub struct Regex {
    program: Vec<Inst>,
    classes: Vec<Class>,
}

impl Regex {
    /// The compiled program's instruction count, for the tests and the status line.
    #[cfg_attr(not(test), allow(dead_code))] // read by the engine tests
    pub fn instruction_count(&self) -> usize {
        self.program.len()
    }

    /// Finds the leftmost match at or after `start` in one line.
    ///
    /// Returns `Ok(None)` for no match and `Err` when the pattern exceeds the step budget. The
    /// error is a value, so a caller can paint it in the find bar instead of crashing.
    pub fn find(
        &self,
        text: &[char],
        start: usize,
        case_sensitive: bool,
    ) -> Result<Option<(usize, usize)>, String> {
        if start > text.len() {
            return Ok(None);
        }
        for begin in start..=text.len() {
            let mut machine = Machine {
                text,
                case_sensitive,
                program: &self.program,
                classes: &self.classes,
                steps: 0,
            };
            if let Some(end) = machine.run(begin)? {
                return Ok(Some((begin, end)));
            }
        }
        Ok(None)
    }

    /// Every non-overlapping match in one line, left to right.
    ///
    /// A zero-length match advances the search by one character so a pattern such as `a*` cannot
    /// produce an unbounded list of empty matches.
    pub fn find_all(
        &self,
        text: &[char],
        case_sensitive: bool,
    ) -> Result<Vec<(usize, usize)>, String> {
        let mut found = Vec::new();
        let mut at = 0usize;
        while at <= text.len() {
            match self.find(text, at, case_sensitive)? {
                Some((start, end)) => {
                    found.push((start, end));
                    at = if end > start { end } else { start + 1 };
                }
                None => break,
            }
        }
        Ok(found)
    }
}

/// The compiled engine has no interior mutability; the step counter belongs to one attempt.
struct Machine<'a> {
    text: &'a [char],
    case_sensitive: bool,
    program: &'a [Inst],
    classes: &'a [Class],
    steps: usize,
}

impl Machine<'_> {
    fn run(&mut self, start: usize) -> Result<Option<usize>, String> {
        // Depth-first backtracking with an explicit stack: no recursion, so a large program can
        // never overflow the thread stack while answering a find.
        let mut stack: Vec<(usize, usize)> = vec![(0, start)];
        while let Some((mut pc, mut pos)) = stack.pop() {
            loop {
                self.steps += 1;
                if self.steps > STEP_BUDGET {
                    return Err(
                        "Pattern is too complex to evaluate safely; simplify it.".to_string()
                    );
                }
                match self.program.get(pc) {
                    None => break,
                    Some(Inst::Match) => return Ok(Some(pos)),
                    Some(Inst::Char(expected)) => {
                        let Some(ch) = self.text.get(pos) else {
                            break;
                        };
                        if !char_eq(*ch, *expected, self.case_sensitive) {
                            break;
                        }
                        pos += 1;
                        pc += 1;
                    }
                    Some(Inst::Any) => {
                        if pos >= self.text.len() {
                            break;
                        }
                        pos += 1;
                        pc += 1;
                    }
                    Some(Inst::Class(index)) => {
                        let Some(ch) = self.text.get(pos) else {
                            break;
                        };
                        let Some(class) = self.classes.get(*index) else {
                            break;
                        };
                        if !class.matches(*ch, self.case_sensitive) {
                            break;
                        }
                        pos += 1;
                        pc += 1;
                    }
                    Some(Inst::Start) => {
                        if pos != 0 {
                            break;
                        }
                        pc += 1;
                    }
                    Some(Inst::End) => {
                        if pos != self.text.len() {
                            break;
                        }
                        pc += 1;
                    }
                    Some(Inst::Jump(target)) => pc = *target,
                    Some(Inst::Split(first, second)) => {
                        // Left branch first: push the right branch and continue with the left.
                        stack.push((*second, pos));
                        pc = *first;
                    }
                }
            }
        }
        Ok(None)
    }
}

fn char_eq(left: char, right: char, case_sensitive: bool) -> bool {
    if case_sensitive {
        left == right
    } else {
        left.to_ascii_lowercase() == right.to_ascii_lowercase()
    }
}

/// Compiles `pattern`, reporting an invalid pattern as a readable error.
pub fn compile(pattern: &str) -> Result<Regex, String> {
    let chars: Vec<char> = pattern.chars().collect();
    if chars.len() > MAX_PATTERN_CHARS {
        return Err(format!(
            "Pattern is longer than {MAX_PATTERN_CHARS} characters."
        ));
    }
    if chars.is_empty() {
        return Err("Pattern is empty.".to_string());
    }
    let mut parser = Parser {
        chars,
        pos: 0,
        depth: 0,
        classes: Vec::new(),
    };
    let node = parser.parse_alt()?;
    if parser.pos != parser.chars.len() {
        return Err(parser.error("unmatched ')'"));
    }
    let mut program = Vec::new();
    let mut classes = parser.classes;
    let mut emit = Emitter {
        program: &mut program,
        classes: &mut classes,
    };
    emit.emit(&node)?;
    program.push(Inst::Match);
    if program.len() > MAX_INSTRUCTIONS {
        return Err(format!(
            "Pattern expands to {} instructions (limit {MAX_INSTRUCTIONS}).",
            program.len()
        ));
    }
    Ok(Regex { program, classes })
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    depth: usize,
    classes: Vec<Class>,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        Some(ch)
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn error(&self, reason: &str) -> String {
        format!(
            "Invalid regular expression at character {}: {reason}",
            self.pos + 1
        )
    }

    fn enter(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            return Err(self.error("the pattern nests too deeply"));
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn parse_alt(&mut self) -> Result<Node, String> {
        let mut branches = vec![self.parse_concat()?];
        while self.eat('|') {
            branches.push(self.parse_concat()?);
        }
        if branches.len() == 1 {
            Ok(branches.remove(0))
        } else {
            Ok(Node::Alt(branches))
        }
    }

    fn parse_concat(&mut self) -> Result<Node, String> {
        let mut parts = Vec::new();
        while let Some(ch) = self.peek() {
            if ch == ')' || ch == '|' {
                break;
            }
            parts.push(self.parse_repeat()?);
        }
        match parts.len() {
            0 => Ok(Node::Empty),
            1 => Ok(parts.remove(0)),
            _ => Ok(Node::Concat(parts)),
        }
    }

    fn parse_repeat(&mut self) -> Result<Node, String> {
        let atom = self.parse_atom()?;
        let mut node = atom;
        loop {
            let quantifier = match self.peek() {
                Some('*') => {
                    self.pos += 1;
                    Some((0, None))
                }
                Some('+') => {
                    self.pos += 1;
                    Some((1, None))
                }
                Some('?') => {
                    self.pos += 1;
                    Some((0, Some(1)))
                }
                Some('{') => Some(self.parse_bounds()?),
                _ => None,
            };
            let Some((min, max)) = quantifier else {
                break;
            };
            node = Node::Repeat {
                node: Box::new(node),
                min,
                max,
            };
            // A lazy/possessive marker only changes preference order, not the set of matches;
            // the engine is greedy, so the marker is accepted and ignored.
            if self.peek() == Some('?') || self.peek() == Some('+') {
                self.pos += 1;
            }
        }
        Ok(node)
    }

    fn parse_bounds(&mut self) -> Result<(usize, Option<usize>), String> {
        if !self.eat('{') {
            return Err(self.error("expected '{'"));
        }
        let min = self.parse_number()?;
        if self.eat('}') {
            return Ok((min, Some(min)));
        }
        if !self.eat(',') {
            return Err(self.error("a repetition needs a comma or '}'"));
        }
        if self.eat('}') {
            return Ok((min, None));
        }
        let max = self.parse_number()?;
        if !self.eat('}') {
            return Err(self.error("unclosed repetition"));
        }
        if max < min {
            return Err(self.error("a repetition's maximum is below its minimum"));
        }
        Ok((min, Some(max)))
    }

    fn parse_number(&mut self) -> Result<usize, String> {
        let start = self.pos;
        let mut value = 0usize;
        while let Some(ch) = self.peek() {
            let Some(digit) = ch.to_digit(10) else {
                break;
            };
            self.pos += 1;
            value = value.saturating_mul(10).saturating_add(digit as usize);
            if value > MAX_REPEAT {
                return Err(self.error("a repetition count is too large"));
            }
        }
        if self.pos == start {
            return Err(self.error("a repetition needs a number"));
        }
        Ok(value)
    }

    fn parse_atom(&mut self) -> Result<Node, String> {
        let Some(ch) = self.next() else {
            return Err(self.error("unexpected end of pattern"));
        };
        match ch {
            '(' => {
                self.enter()?;
                if self.eat('?') {
                    if !self.eat(':') {
                        return Err(self.error("only the (?:...) group prefix is supported"));
                    }
                }
                let inner = self.parse_alt()?;
                if !self.eat(')') {
                    return Err(self.error("unclosed group"));
                }
                self.leave();
                Ok(inner)
            }
            '[' => {
                self.enter()?;
                let class = self.parse_class()?;
                self.leave();
                let index = self.classes.len();
                self.classes.push(class);
                Ok(Node::Class(index))
            }
            '.' => Ok(Node::Any),
            '^' => Ok(Node::Start),
            '$' => Ok(Node::End),
            '\\' => self.parse_escape_atom(),
            '*' | '+' | '?' => Err(self.error("nothing to repeat")),
            ')' => Err(self.error("unmatched ')'")),
            '{' => Err(self.error("'{' must follow an expression")),
            ']' | '}' => Err(self.error("unmatched bracket")),
            other => Ok(Node::Char(other)),
        }
    }

    fn parse_escape_atom(&mut self) -> Result<Node, String> {
        let Some(ch) = self.next() else {
            return Err(self.error("a trailing backslash has no escape"));
        };
        match ch {
            'd' | 'D' | 'w' | 'W' | 's' | 'S' => {
                let item = shorthand(ch);
                let index = self.classes.len();
                self.classes.push(Class {
                    negated: false,
                    items: vec![item],
                });
                Ok(Node::Class(index))
            }
            'n' => Ok(Node::Char('\n')),
            't' => Ok(Node::Char('\t')),
            'r' => Ok(Node::Char('\r')),
            other => Ok(Node::Char(other)),
        }
    }

    fn parse_class(&mut self) -> Result<Class, String> {
        let negated = self.eat('^');
        let mut items: Vec<ClassItem> = Vec::new();
        let mut first = true;
        loop {
            let Some(ch) = self.next() else {
                return Err(self.error("unclosed character class"));
            };
            if ch == ']' && !first {
                break;
            }
            first = false;
            if ch == '\\' {
                let Some(escaped) = self.next() else {
                    return Err(self.error("a trailing backslash has no escape"));
                };
                match escaped {
                    'd' | 'D' | 'w' | 'W' | 's' | 'S' => items.push(shorthand(escaped)),
                    'n' => items.push(ClassItem::Literal('\n')),
                    't' => items.push(ClassItem::Literal('\t')),
                    'r' => items.push(ClassItem::Literal('\r')),
                    other => items.push(ClassItem::Literal(other)),
                }
                continue;
            }
            // A '-' forms a range only when it has a literal endpoint before the closing ']'.
            if self.peek() == Some('-')
                && self
                    .chars
                    .get(self.pos + 1)
                    .is_some_and(|next| *next != ']')
            {
                self.pos += 1;
                let Some(end) = self.next() else {
                    return Err(self.error("unclosed character class"));
                };
                let end = if end == '\\' {
                    let Some(escaped) = self.next() else {
                        return Err(self.error("a trailing backslash has no escape"));
                    };
                    match escaped {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        other => other,
                    }
                } else {
                    end
                };
                if end < ch {
                    return Err(self.error("a character range is out of order"));
                }
                items.push(ClassItem::Range(ch, end));
                continue;
            }
            items.push(ClassItem::Literal(ch));
        }
        if items.is_empty() {
            return Err(self.error("an empty character class matches nothing"));
        }
        Ok(Class { negated, items })
    }
}

fn shorthand(ch: char) -> ClassItem {
    match ch {
        'd' => ClassItem::Digit(false),
        'D' => ClassItem::Digit(true),
        'w' => ClassItem::Word(false),
        'W' => ClassItem::Word(true),
        's' => ClassItem::Space(false),
        'S' => ClassItem::Space(true),
        _ => ClassItem::Literal(ch),
    }
}

struct Emitter<'a> {
    program: &'a mut Vec<Inst>,
    classes: &'a mut [Class],
}

impl Emitter<'_> {
    fn emit(&mut self, node: &Node) -> Result<(), String> {
        if self.program.len() > MAX_INSTRUCTIONS {
            return Err(format!(
                "Pattern expands to more than {MAX_INSTRUCTIONS} instructions."
            ));
        }
        match node {
            Node::Empty => {}
            Node::Char(ch) => self.program.push(Inst::Char(*ch)),
            Node::Any => self.program.push(Inst::Any),
            Node::Class(index) => {
                if self.classes.get(*index).is_none() {
                    return Err("Internal pattern error: missing character class.".to_string());
                }
                self.program.push(Inst::Class(*index));
            }
            Node::Start => self.program.push(Inst::Start),
            Node::End => self.program.push(Inst::End),
            Node::Concat(parts) => {
                for part in parts {
                    self.emit(part)?;
                }
            }
            Node::Alt(branches) => {
                let mut jumps = Vec::new();
                for (index, branch) in branches.iter().enumerate() {
                    if index + 1 < branches.len() {
                        let split = self.program.len();
                        self.program.push(Inst::Split(split + 1, 0));
                        self.emit(branch)?;
                        jumps.push(self.program.len());
                        self.program.push(Inst::Jump(0));
                        let next_branch = self.program.len();
                        if let Some(Inst::Split(_, second)) = self.program.get_mut(split) {
                            *second = next_branch;
                        }
                    } else {
                        self.emit(branch)?;
                    }
                }
                let end = self.program.len();
                for jump in jumps {
                    if let Some(Inst::Jump(target)) = self.program.get_mut(jump) {
                        *target = end;
                    }
                }
            }
            Node::Repeat { node, min, max } => {
                for _ in 0..*min {
                    self.emit(node)?;
                }
                match max {
                    None => {
                        let split = self.program.len();
                        self.program.push(Inst::Split(split + 1, 0));
                        self.emit(node)?;
                        self.program.push(Inst::Jump(split));
                        let after = self.program.len();
                        if let Some(Inst::Split(_, second)) = self.program.get_mut(split) {
                            *second = after;
                        }
                    }
                    Some(max) => {
                        let mut jumps = Vec::new();
                        for _ in *min..*max {
                            let split = self.program.len();
                            self.program.push(Inst::Split(split + 1, 0));
                            self.emit(node)?;
                            jumps.push(self.program.len());
                            self.program.push(Inst::Jump(0));
                            let after = self.program.len();
                            if let Some(Inst::Split(_, second)) = self.program.get_mut(split) {
                                *second = after;
                            }
                        }
                        let end = self.program.len();
                        for jump in jumps {
                            if let Some(Inst::Jump(target)) = self.program.get_mut(jump) {
                                *target = end;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    fn all(pattern: &str, text: &str) -> Vec<(usize, usize)> {
        compile(pattern)
            .expect("valid pattern")
            .find_all(&chars(text), true)
            .expect("the pattern evaluates")
    }

    #[test]
    fn literals_alternation_and_quantifiers_match_the_expected_spans() {
        assert!(compile("ab*c").expect("valid").instruction_count() > 0);
        assert_eq!(all("b.", "abc"), vec![(1, 3)]);
        assert_eq!(all("a|bc", "zbc"), vec![(1, 3)]);
        assert_eq!(all("ab*c", "ac abc abbc"), vec![(0, 2), (3, 6), (7, 11)]);
        assert_eq!(all("ab+c", "ac abc abbc"), vec![(3, 6), (7, 11)]);
        assert_eq!(all("ab?c", "ac abc abbc"), vec![(0, 2), (3, 6)]);
        assert_eq!(all("a{2}", "a aa aaa"), vec![(2, 4), (5, 7)]);
        assert_eq!(all("a{2,3}", "a aa aaaa"), vec![(2, 4), (5, 8)]);
        assert_eq!(all("a{2,}", "a aaaa"), vec![(2, 6)]);
    }

    #[test]
    fn classes_shorthands_and_anchors_are_line_local() {
        assert_eq!(all("[a-c]+", "xxabcxx"), vec![(2, 5)]);
        assert_eq!(all("[^a-c]+", "abcxyz"), vec![(3, 6)]);
        assert_eq!(all("\\d+", "ab123cd45"), vec![(2, 5), (7, 9)]);
        assert_eq!(all("\\w+", "a_b c"), vec![(0, 3), (4, 5)]);
        assert_eq!(all("\\s+", "a  b"), vec![(1, 3)]);
        assert_eq!(all("^ab", "abab"), vec![(0, 2)]);
        assert_eq!(all("ab$", "abab"), vec![(2, 4)]);
        assert_eq!(all("[\\d-]+", "a1-2b"), vec![(1, 4)]);
    }

    #[test]
    fn case_insensitive_matching_is_opt_in() {
        let pattern = compile("aBc").expect("valid");
        assert_eq!(
            pattern.find(&chars("xABCx"), 0, true).expect("runs"),
            None,
            "case-sensitive keeps the pattern's case"
        );
        assert_eq!(
            pattern.find(&chars("xABCx"), 0, false).expect("runs"),
            Some((1, 4)),
            "case-insensitive folds ASCII"
        );
        assert_eq!(all("[A-Z]+", "abc"), Vec::new());
        let class = compile("[A-Z]+").expect("valid");
        assert_eq!(
            class.find(&chars("abc"), 0, false).expect("runs"),
            Some((0, 3)),
            "a range folds both directions when case-insensitive"
        );
    }

    #[test]
    fn zero_length_matches_advance_and_never_loop() {
        assert_eq!(all("a*", "ba"), vec![(0, 0), (1, 2), (2, 2)]);
        assert_eq!(all("^", "ab"), vec![(0, 0)]);
        assert_eq!(all("$", "ab"), vec![(2, 2)]);
    }

    #[test]
    fn invalid_patterns_are_errors_and_never_panic() {
        for pattern in [
            "(", ")", "[", "[]", "*", "+", "?", "a{", "a{2,1}", "\\", "a\\", "(?=a)", "[z-a]",
            "a{1000}", "(a", "[a-",
        ] {
            let result = compile(pattern);
            assert!(
                result.is_err(),
                "pattern {pattern:?} must be rejected, got {result:?}"
            );
        }
        // A too-complex pattern is a typed error from the matcher, not a hang and not a panic.
        let pattern = compile("(a?)*b").expect("the pattern compiles");
        let error = pattern
            .find(&chars(&"a".repeat(200)), 0, true)
            .expect_err("the step budget trips");
        assert!(error.contains("too complex"), "{error}");
    }

    #[test]
    fn a_pattern_that_is_too_large_is_rejected() {
        let pattern = "a".repeat(MAX_PATTERN_CHARS + 1);
        assert!(compile(&pattern).is_err());
        let repeated = format!("a{{{MAX_REPEAT}}}");
        assert!(compile(&repeated).is_ok(), "the maximum count is accepted");
        let too_many = format!("a{{{}}}", MAX_REPEAT + 1);
        assert!(compile(&too_many).is_err());
    }

    #[test]
    fn non_capturing_groups_work_and_captures_are_plain_groups() {
        assert_eq!(all("(?:ab)+", "ababx"), vec![(0, 4)]);
        assert_eq!(all("(ab)+", "ababx"), vec![(0, 4)]);
        assert_eq!(all("(a|b)c", "ac bc"), vec![(0, 2), (3, 5)]);
    }
}
