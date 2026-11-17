//! Number patterns, such as `CAPA-{year}-{seq:4}` or `QR-{rand:4}`.

use std::fmt::Write;

/// What reading a number back found so far.
#[derive(Default)]
struct Read {
    scope: String,
    sequence: Option<u64>,
    parent: Option<String>,
}

/// A parsed number pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumberPattern {
    parts: Vec<Part>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    Literal(String),
    Year,
    Month,
    Parent,
    Sequence { width: usize },
    Random { width: usize },
}

/// What an akt number is rendered from, besides its sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberContext<'a> {
    /// The year of the akt's creation, in UTC.
    pub year: i32,
    /// The month of the akt's creation, in UTC, from 1.
    pub month: u32,
    /// The akt number of the container the akt is created in, for
    /// `{parent}`. A pattern that [uses it](NumberPattern::uses_parent) must
    /// be given one.
    pub parent: Option<&'a str>,
    /// The digits drawn for each `{rand:N}`, in order
    /// (`akt.number.random`). A pattern's scope is rendered without them,
    /// as placeholders, until they are given.
    pub random: &'a [String],
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PatternError {
    #[error(
        "`{{{0}}}` is not a placeholder; use {{year}}, {{month}}, {{parent}}, {{seq}}, {{seq:N}} or {{rand:N}}"
    )]
    UnknownPlaceholder(String),
    #[error("a `{{` is never closed")]
    Unclosed,
    #[error("a `}}` has no `{{`")]
    Unopened,
    #[error("the pattern can hold one {{seq}} or {{seq:N}}, not {0}")]
    Sequences(usize),
    #[error("the pattern needs a {{seq}}, {{seq:N}} or {{rand:N}} to tell akter apart")]
    Nothing,
    #[error("{{seq:N}} and {{rand:N}} take a width from 1 to 12")]
    Width,
}

const MAX_WIDTH: usize = 12;

impl NumberPattern {
    /// Parses `pattern`.
    // r[impl akt.number.pattern]
    pub fn parse(pattern: &str) -> Result<Self, PatternError> {
        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut rest = pattern;
        while let Some(i) = rest.find(['{', '}']) {
            literal.push_str(&rest[..i]);
            if rest[i..].starts_with('}') {
                return Err(PatternError::Unopened);
            }
            let after = &rest[i + 1..];
            let end = after.find('}').ok_or(PatternError::Unclosed)?;
            let name = &after[..end];
            if name.contains('{') {
                return Err(PatternError::Unclosed);
            }
            if !literal.is_empty() {
                parts.push(Part::Literal(std::mem::take(&mut literal)));
            }
            parts.push(match name {
                "year" => Part::Year,
                "month" => Part::Month,
                "parent" => Part::Parent,
                "seq" => Part::Sequence { width: 1 },
                "rand" => return Err(PatternError::Width),
                other => {
                    let width = |w: &str| -> Result<usize, PatternError> {
                        let width: usize = w.parse().map_err(|_| PatternError::Width)?;
                        if (1..=MAX_WIDTH).contains(&width) {
                            Ok(width)
                        } else {
                            Err(PatternError::Width)
                        }
                    };
                    if let Some(w) = other.strip_prefix("seq:") {
                        Part::Sequence { width: width(w)? }
                    } else if let Some(w) = other.strip_prefix("rand:") {
                        Part::Random { width: width(w)? }
                    } else {
                        return Err(PatternError::UnknownPlaceholder(other.to_owned()));
                    }
                }
            });
            rest = &after[end + 1..];
        }
        literal.push_str(rest);
        if !literal.is_empty() {
            parts.push(Part::Literal(literal));
        }
        let sequences = parts
            .iter()
            .filter(|p| matches!(p, Part::Sequence { .. }))
            .count();
        if sequences > 1 {
            return Err(PatternError::Sequences(sequences));
        }
        let pattern = Self { parts };
        if sequences == 0 && pattern.random_widths().is_empty() {
            return Err(PatternError::Nothing);
        }
        Ok(pattern)
    }

    /// Whether the pattern holds a `{seq}`, so that akter are counted.
    #[must_use]
    pub fn has_sequence(&self) -> bool {
        self.parts
            .iter()
            .any(|p| matches!(p, Part::Sequence { .. }))
    }

    /// The width of each `{rand:N}`, in order.
    #[must_use]
    pub fn random_widths(&self) -> Vec<usize> {
        self.parts
            .iter()
            .filter_map(|p| match p {
                Part::Random { width } => Some(*width),
                _ => None,
            })
            .collect()
    }

    /// The pattern rendered for `context` as a SQL `LIKE` pattern, escaped
    /// with `\\`: every number it can render for `context`, whatever digits
    /// are drawn. Only for a pattern without a `{seq}`, whose numbers are
    /// as wide as their digits.
    #[must_use]
    pub fn like(&self, context: &NumberContext<'_>) -> String {
        let escape = |text: &str| {
            text.chars().fold(String::new(), |mut out, c| {
                if matches!(c, '%' | '_' | '\\') {
                    out.push('\\');
                }
                out.push(c);
                out
            })
        };
        let mut out = String::new();
        for part in &self.parts {
            match part {
                Part::Literal(text) => out.push_str(&escape(text)),
                Part::Year => {
                    let _ = write!(out, "{:04}", context.year);
                }
                Part::Month => {
                    let _ = write!(out, "{:02}", context.month);
                }
                Part::Parent => out.push_str(&escape(context.parent.unwrap_or_default())),
                Part::Sequence { .. } => out.push('%'),
                Part::Random { width } => out.push_str(&"_".repeat(*width)),
            }
        }
        out
    }

    /// Whether the pattern holds `{parent}`.
    #[must_use]
    pub fn uses_parent(&self) -> bool {
        self.parts.contains(&Part::Parent)
    }

    /// The pattern rendered for `context` with the sequence left as a
    /// placeholder. Akt numbers with the same scope share a sequence, so a
    /// pattern with `{parent}` counts separately in each parent, and one
    /// with `{rand:N}` for each drawing.
    // r[impl akt.number.sequence]
    #[must_use]
    pub fn scope(&self, context: &NumberContext<'_>) -> String {
        self.render_with(context, |out, width| {
            let _ = write!(out, "{{seq:{width}}}");
        })
    }

    /// The akt number for `context` and `sequence`, which a pattern without
    /// a `{seq}` doesn't use.
    #[must_use]
    pub fn render(&self, context: &NumberContext<'_>, sequence: u64) -> String {
        self.render_with(context, |out, width| {
            let _ = write!(out, "{sequence:0width$}");
        })
    }

    fn render_with(
        &self,
        context: &NumberContext<'_>,
        sequence: impl Fn(&mut String, usize),
    ) -> String {
        let mut out = String::new();
        let mut drawn = 0;
        for part in &self.parts {
            match part {
                Part::Literal(text) => out.push_str(text),
                Part::Year => {
                    let _ = write!(out, "{:04}", context.year);
                }
                Part::Month => {
                    let _ = write!(out, "{:02}", context.month);
                }
                Part::Parent => out.push_str(context.parent.unwrap_or_default()),
                Part::Sequence { width } => sequence(&mut out, *width),
                Part::Random { width } => match context.random.get(drawn) {
                    Some(digits) => out.push_str(digits),
                    None => {
                        let _ = write!(out, "{{rand:{width}}}");
                    }
                },
            }
            if matches!(part, Part::Random { .. }) {
                drawn += 1;
            }
        }
        out
    }

    /// Reads `number` back as this pattern rendered it: the counter scope
    /// it was numbered in, as [`NumberPattern::scope`] gives it, and its
    /// sequence. `None` if the pattern can't have rendered it.
    // r[impl bundle.import.counters]
    #[must_use]
    pub fn read(&self, number: &str) -> Option<(String, u64)> {
        let mut read = Read::default();
        self.read_from(0, number, &mut read, &|_| true)
            .then(|| read.sequence.map(|s| (read.scope, s)))
            .flatten()
    }

    /// The parent `number` was numbered after, of those `is_parent` says
    /// could be: `{parent}` matches any text, so a number such as
    /// `PLAN-001-T01-T02` reads back as numbered after `PLAN-001-T01`, not
    /// `PLAN-001`, only if `PLAN-001-T01` is a parent. `None` if the pattern
    /// has no `{parent}` or no such parent.
    #[must_use]
    pub fn parent_of(&self, number: &str, is_parent: &dyn Fn(&str) -> bool) -> Option<String> {
        if !self.uses_parent() {
            return None;
        }
        let mut read = Read::default();
        self.read_from(0, number, &mut read, is_parent)
            .then_some(read.parent)
            .flatten()
    }

    /// Whether `rest` matches the parts from `index` on, with a parent
    /// `accept` takes, writing the scope, sequence and parent of the first
    /// way it does.
    fn read_from(
        &self,
        index: usize,
        rest: &str,
        read: &mut Read,
        accept: &dyn Fn(&str) -> bool,
    ) -> bool {
        let Some(part) = self.parts.get(index) else {
            return rest.is_empty() && read.parent.as_deref().is_none_or(accept);
        };
        let digits =
            |n: usize| rest.len() >= n && rest.as_bytes()[..n].iter().all(u8::is_ascii_digit);
        let mut attempt = |taken: usize, written: &str, seq: Option<u64>| {
            let kept = (read.scope.len(), read.sequence, read.parent.clone());
            read.scope.push_str(written);
            if seq.is_some() {
                read.sequence = seq;
            }
            if *part == Part::Parent {
                read.parent = Some(written.to_owned());
            }
            if self.read_from(index + 1, &rest[taken..], read, accept) {
                return true;
            }
            read.scope.truncate(kept.0);
            (read.sequence, read.parent) = (kept.1, kept.2);
            false
        };
        match part {
            Part::Literal(text) => {
                rest.starts_with(text.as_str()) && attempt(text.len(), text, None)
            }
            Part::Year => digits(4) && attempt(4, &rest[..4], None),
            Part::Month => digits(2) && attempt(2, &rest[..2], None),
            // Drawn digits are part of the scope, as a year is.
            Part::Random { width } => digits(*width) && attempt(*width, &rest[..*width], None),
            // A parent's number is any text, as short as the rest allows.
            Part::Parent => (1..=rest.len())
                .filter(|&n| rest.is_char_boundary(n))
                .any(|n| attempt(n, &rest[..n], None)),
            Part::Sequence { width } => {
                let run = rest.bytes().take_while(u8::is_ascii_digit).count();
                // Rendered at least `width` wide; wider once it outgrows it.
                (*width..=run).rev().any(|n| {
                    let value = rest[..n].parse().ok();
                    value.is_some() && attempt(n, &format!("{{seq:{width}}}"), value)
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(year: i32, month: u32) -> NumberContext<'static> {
        NumberContext {
            year,
            month,
            parent: None,
            random: &[],
        }
    }

    // r[verify akt.number.pattern]
    #[test]
    fn renders_placeholders() {
        let pattern = NumberPattern::parse("CAPA-{year}-{seq:4}").unwrap();
        assert_eq!(pattern.render(&at(2026, 11), 42), "CAPA-2026-0042");
        assert_eq!(pattern.render(&at(2026, 11), 123_456), "CAPA-2026-123456");
        assert!(!pattern.uses_parent());
        let pattern = NumberPattern::parse("{year}{month}/{seq}").unwrap();
        assert_eq!(pattern.render(&at(2026, 3), 7), "202603/7");
    }

    // r[verify akt.number.pattern]
    #[test]
    fn renders_the_parent() {
        let pattern = NumberPattern::parse("{parent}-RC-{seq:4}").unwrap();
        assert!(pattern.uses_parent());
        let context = NumberContext {
            parent: Some("RELEASE-2026-0001"),
            ..at(2026, 11)
        };
        assert_eq!(pattern.render(&context, 1), "RELEASE-2026-0001-RC-0001");
    }

    // r[verify akt.number.pattern]
    #[test]
    fn refuses_invalid_patterns() {
        for (pattern, error) in [
            ("CAPA", PatternError::Nothing),
            ("A-{rand}", PatternError::Width),
            ("A-{rand:0}", PatternError::Width),
            ("A-{rand:13}", PatternError::Width),
            ("{seq}-{seq}", PatternError::Sequences(2)),
            (
                "A-{day}-{seq}",
                PatternError::UnknownPlaceholder("day".to_owned()),
            ),
            ("A-{seq", PatternError::Unclosed),
            ("A}-{seq}", PatternError::Unopened),
            ("A-{seq:0}", PatternError::Width),
            ("A-{seq:x}", PatternError::Width),
            ("A-{seq:13}", PatternError::Width),
        ] {
            assert_eq!(NumberPattern::parse(pattern), Err(error), "{pattern}");
        }
    }

    // r[verify akt.number.sequence]
    #[test]
    fn scopes_by_everything_but_the_sequence() {
        let pattern = NumberPattern::parse("CAPA-{year}-{seq:4}").unwrap();
        assert_eq!(pattern.scope(&at(2026, 1)), pattern.scope(&at(2026, 12)));
        assert_ne!(pattern.scope(&at(2026, 1)), pattern.scope(&at(2027, 1)));
        assert_eq!(pattern.scope(&at(2026, 1)), "CAPA-2026-{seq:4}");
    }

    // r[verify akt.number.sequence]
    #[test]
    fn counts_separately_in_each_parent() {
        let pattern = NumberPattern::parse("{parent}-HOTFIX-{seq:4}").unwrap();
        let first = NumberContext {
            parent: Some("RELEASE-2026-0001"),
            ..at(2026, 1)
        };
        let second = NumberContext {
            parent: Some("RELEASE-2026-0002"),
            ..at(2026, 1)
        };
        assert_ne!(pattern.scope(&first), pattern.scope(&second));
        assert_eq!(pattern.scope(&first), "RELEASE-2026-0001-HOTFIX-{seq:4}");
    }

    // r[verify bundle.import.counters]
    #[test]
    fn reads_a_number_back_into_its_scope_and_sequence() {
        let read =
            |pattern: &str, number: &str| NumberPattern::parse(pattern).unwrap().read(number);
        let scope_of = |pattern: &str, context: &NumberContext<'_>| {
            NumberPattern::parse(pattern).unwrap().scope(context)
        };
        assert_eq!(
            read("CAPA-{year}-{seq:4}", "CAPA-2026-0042"),
            Some((scope_of("CAPA-{year}-{seq:4}", &at(2026, 1)), 42))
        );
        assert_eq!(
            read("CAPA-{year}-{seq:4}", "CAPA-2026-123456").map(|r| r.1),
            Some(123_456)
        );
        let parent = NumberContext {
            parent: Some("REL-0001"),
            ..at(2026, 11)
        };
        assert_eq!(
            read("{parent}-RC-{seq:2}", "REL-0001-RC-07"),
            Some((scope_of("{parent}-RC-{seq:2}", &parent), 7))
        );
        assert_eq!(
            read("{year}{month}/{seq}", "202603/7").map(|r| r.1),
            Some(7)
        );
        assert_eq!(read("CAPA-{year}-{seq:4}", "CAPA-2026-042"), None);
        assert_eq!(read("CAPA-{seq}", "TASK-1"), None);
        assert_eq!(read("CAPA-{seq}", "CAPA-1x"), None);
    }

    #[test]
    fn reads_the_parent_a_number_was_made_from() {
        let task = NumberPattern::parse("{parent}-T{seq:2}").unwrap();
        let parents = ["PLAN-001", "PLAN-001-T01"];
        let is_parent = |n: &str| parents.contains(&n);
        assert_eq!(
            task.parent_of("PLAN-001-T07", &is_parent).as_deref(),
            Some("PLAN-001")
        );
        assert_eq!(
            task.parent_of("PLAN-001-T100", &is_parent).as_deref(),
            Some("PLAN-001")
        );
        assert_eq!(
            task.parent_of("PLAN-001-T01-T02", &is_parent).as_deref(),
            Some("PLAN-001-T01")
        );
        assert_eq!(task.parent_of("PLAN-002-T01", &is_parent), None);
        let plan = NumberPattern::parse("PLAN-{seq:3}").unwrap();
        assert_eq!(plan.parent_of("PLAN-001", &|_| true), None);
    }

    // r[verify akt.number.pattern]
    // r[verify akt.number.random]
    #[test]
    fn renders_and_reads_random_digits() {
        let drawn = ["4821".to_owned()];
        let context = NumberContext {
            random: &drawn,
            ..at(2026, 11)
        };
        let alone = NumberPattern::parse("QR-{rand:4}").unwrap();
        assert!(!alone.has_sequence());
        assert_eq!(alone.random_widths(), [4]);
        assert_eq!(alone.render(&context, 0), "QR-4821");
        assert_eq!(alone.like(&at(2026, 11)), "QR-____");
        // No sequence, so nothing to count.
        assert_eq!(alone.read("QR-4821"), None);

        let both = NumberPattern::parse("QR_{year}-{rand:4}-{seq:2}").unwrap();
        assert!(both.has_sequence());
        assert_eq!(both.render(&context, 3), "QR_2026-4821-03");
        // The drawn digits are part of the counter's scope.
        assert_eq!(both.scope(&context), "QR_2026-4821-{seq:2}");
        assert_eq!(
            both.read("QR_2026-4821-03"),
            Some(("QR_2026-4821-{seq:2}".to_owned(), 3))
        );
        assert_eq!(both.read("QR_2026-482-03"), None);
        assert_eq!(both.like(&at(2026, 1)), "QR\\_2026-____-%");
    }
}
