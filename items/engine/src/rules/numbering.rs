//! Akt numbers: the counter scope a new akt is numbered in, and its number
//! (`akt.number.*`).

use chrono::{DateTime, Datelike as _, Utc};
use kuria_config::{AktType, NumberContext};

fn context(at: DateTime<Utc>, parent: Option<&str>) -> NumberContext<'_> {
    NumberContext {
        year: at.year(),
        month: at.month(),
        parent,
    }
}

/// The name of the counter an akt of `ty` created at `at` in `parent` takes
/// its sequence from: akter whose numbers share everything but the
/// sequence share a counter.
// r[impl akt.number.sequence]
#[must_use]
pub fn counter(ty: &AktType, at: DateTime<Utc>, parent: Option<&str>) -> String {
    format!("number:{}", ty.number().scope(&context(at, parent)))
}

/// The number of an akt of `ty` created at `at` in `parent`, with
/// `sequence` from its counter.
// r[impl akt.number.pattern]
#[must_use]
pub fn render(ty: &AktType, at: DateTime<Utc>, parent: Option<&str>, sequence: u64) -> String {
    ty.number().render(&context(at, parent), sequence)
}

/// The next number for an akt of `ty` created at `at` in `parent`, one past
/// the highest sequence of its counter among `existing` numbers, for a
/// store that keeps no counter (`cli.local.numbers`).
// r[impl cli.local.numbers]
#[must_use]
pub fn next_from<'a>(
    ty: &AktType,
    at: DateTime<Utc>,
    parent: Option<&str>,
    existing: impl IntoIterator<Item = &'a str>,
) -> String {
    let scope = ty.number().scope(&context(at, parent));
    let highest = existing
        .into_iter()
        .filter_map(|number| ty.number().read(number))
        .filter(|(s, _)| *s == scope)
        .map(|(_, sequence)| sequence)
        .max()
        .unwrap_or(0);
    render(ty, at, parent, highest + 1)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone as _;
    use kuria_config::{AktTypeDocument, validate};
    use serde_json::json;

    use super::*;

    fn ty(pattern: &str) -> AktType {
        let document: AktTypeDocument = serde_json::from_value(json!({
            "key": "t", "name": "T", "number": pattern, "created_in": ["plan"],
            "fields": [], "states": [{ "key": "open", "name": "Open", "initial": true }]
        }))
        .unwrap();
        validate(document).unwrap()
    }

    // r[verify cli.local.numbers]
    // r[verify akt.number.sequence]
    #[test]
    fn numbers_follow_the_highest_of_their_scope() {
        let at = Utc.with_ymd_and_hms(2026, 11, 13, 0, 0, 0).unwrap();
        let task = ty("{parent}-T{seq:2}");
        let existing = ["PLAN-001-T01", "PLAN-001-T07", "PLAN-002-T03", "PLAN-001"];
        assert_eq!(
            next_from(&task, at, Some("PLAN-001"), existing),
            "PLAN-001-T08"
        );
        assert_eq!(
            next_from(&task, at, Some("PLAN-002"), existing),
            "PLAN-002-T04"
        );
        assert_eq!(
            next_from(&task, at, Some("PLAN-003"), existing),
            "PLAN-003-T01"
        );
        let capa = ty("CAPA-{year}-{seq:4}");
        assert_eq!(
            next_from(&capa, at, None, ["CAPA-2025-0009", "CAPA-2026-0002"]),
            "CAPA-2026-0003"
        );
        assert_eq!(counter(&capa, at, None), "number:CAPA-2026-{seq:4}");
        assert_eq!(render(&capa, at, None, 12), "CAPA-2026-0012");
    }
}
