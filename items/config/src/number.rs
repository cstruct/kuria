//! Number patterns, such as `CAPA-{year}-{seq:4}`.

use std::fmt::Write;

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
    Sequence { width: usize },
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PatternError {
    #[error("`{{{0}}}` is not a placeholder; use {{year}}, {{month}}, {{seq}} or {{seq:N}}")]
    UnknownPlaceholder(String),
    #[error("a `{{` is never closed")]
    Unclosed,
    #[error("a `}}` has no `{{`")]
    Unopened,
    #[error("the pattern needs exactly one {{seq}} or {{seq:N}}, not {0}")]
    Sequences(usize),
    #[error("{{seq:N}} takes a width from 1 to 12")]
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
                "seq" => Part::Sequence { width: 1 },
                other => match other.strip_prefix("seq:") {
                    Some(width) => {
                        let width: usize = width.parse().map_err(|_| PatternError::Width)?;
                        if !(1..=MAX_WIDTH).contains(&width) {
                            return Err(PatternError::Width);
                        }
                        Part::Sequence { width }
                    }
                    None => return Err(PatternError::UnknownPlaceholder(other.to_owned())),
                },
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
        if sequences != 1 {
            return Err(PatternError::Sequences(sequences));
        }
        Ok(Self { parts })
    }

    /// The pattern rendered for `year` and `month` with the sequence left as
    /// a placeholder. Akt numbers with the same scope share a sequence.
    // r[impl akt.number.sequence]
    #[must_use]
    pub fn scope(&self, year: i32, month: u32) -> String {
        self.render_with(year, month, |out, width| {
            let _ = write!(out, "{{seq:{width}}}");
        })
    }

    /// The akt number for `year`, `month` and `sequence`.
    #[must_use]
    pub fn render(&self, year: i32, month: u32, sequence: u64) -> String {
        self.render_with(year, month, |out, width| {
            let _ = write!(out, "{sequence:0width$}");
        })
    }

    fn render_with(&self, year: i32, month: u32, sequence: impl Fn(&mut String, usize)) -> String {
        let mut out = String::new();
        for part in &self.parts {
            match part {
                Part::Literal(text) => out.push_str(text),
                Part::Year => {
                    let _ = write!(out, "{year:04}");
                }
                Part::Month => {
                    let _ = write!(out, "{month:02}");
                }
                Part::Sequence { width } => sequence(&mut out, *width),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify akt.number.pattern]
    #[test]
    fn renders_placeholders() {
        let pattern = NumberPattern::parse("CAPA-{year}-{seq:4}").unwrap();
        assert_eq!(pattern.render(2026, 11, 42), "CAPA-2026-0042");
        assert_eq!(pattern.render(2026, 11, 123_456), "CAPA-2026-123456");
        let pattern = NumberPattern::parse("{year}{month}/{seq}").unwrap();
        assert_eq!(pattern.render(2026, 3, 7), "202603/7");
    }

    // r[verify akt.number.pattern]
    #[test]
    fn refuses_invalid_patterns() {
        for (pattern, error) in [
            ("CAPA", PatternError::Sequences(0)),
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
        assert_eq!(pattern.scope(2026, 1), pattern.scope(2026, 12));
        assert_ne!(pattern.scope(2026, 1), pattern.scope(2027, 1));
        assert_eq!(pattern.scope(2026, 1), "CAPA-2026-{seq:4}");
    }
}
