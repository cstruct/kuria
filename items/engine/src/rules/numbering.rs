//! Akt numbers: the counter scope a new akt is numbered in, the digits
//! drawn for its `{rand:N}`, and its number (`akt.number.*`).

use std::collections::BTreeSet;

use chrono::{DateTime, Datelike as _, Utc};
use kuria_config::{AktType, NumberContext};

/// A source of random numbers, each as likely as any other.
pub type Random<'a> = &'a mut dyn FnMut() -> u64;

/// Draws before giving up on a pattern without a `{seq}` whose numbers are
/// too many to list.
const DRAWS: usize = 64;
/// The most digits a pattern without a `{seq}` is listed in full for, once
/// drawing at random keeps finding numbers taken.
const LISTED: u32 = 6;

fn context<'a>(
    at: DateTime<Utc>,
    parent: Option<&'a str>,
    random: &'a [String],
) -> NumberContext<'a> {
    NumberContext {
        year: at.year(),
        month: at.month(),
        parent,
        random,
    }
}

/// A number below `n`, each as likely as any other.
fn below(n: u64, random: Random<'_>) -> u64 {
    // Rejecting the top of the range that doesn't divide by `n`.
    let zone = u64::MAX - u64::MAX % n;
    loop {
        let r = random();
        if r < zone {
            return r % n;
        }
    }
}

/// The digits for each of `ty`'s `{rand:N}`, drawn at random.
// r[impl akt.number.random]
#[must_use]
pub fn draw(ty: &AktType, random: Random<'_>) -> Vec<String> {
    ty.number()
        .random_widths()
        .into_iter()
        .map(|width| {
            (0..width)
                .map(|_| {
                    char::from_digit(u32::try_from(below(10, random)).unwrap_or(0), 10)
                        .unwrap_or('0')
                })
                .collect()
        })
        .collect()
}

/// A `{rand:N}`'s width, which is at most 12 (`akt.number.pattern`).
fn wide(width: usize) -> u32 {
    u32::try_from(width).expect("a width is at most 12")
}

/// `index` as the digits of `widths`, the first the most significant.
fn digits_of(mut index: u64, widths: &[usize]) -> Vec<String> {
    let mut drawn: Vec<String> = widths
        .iter()
        .rev()
        .map(|&width| {
            let modulus = 10_u64.pow(wide(width));
            let part = index % modulus;
            index /= modulus;
            format!("{part:0width$}")
        })
        .collect();
    drawn.reverse();
    drawn
}

/// The name of the counter an akt of `ty` created at `at` in `parent`, with
/// the digits `random` drawn, takes its sequence from: akter whose numbers
/// share everything but the sequence share a counter.
// r[impl akt.number.sequence]
#[must_use]
pub fn counter(ty: &AktType, at: DateTime<Utc>, parent: Option<&str>, random: &[String]) -> String {
    format!("number:{}", ty.number().scope(&context(at, parent, random)))
}

/// The number of an akt of `ty` created at `at` in `parent`, with the
/// digits `random` drawn and `sequence` from its counter.
// r[impl akt.number.pattern]
#[must_use]
pub fn render(
    ty: &AktType,
    at: DateTime<Utc>,
    parent: Option<&str>,
    random: &[String],
    sequence: u64,
) -> String {
    ty.number().render(&context(at, parent, random), sequence)
}

/// The counter to raise, and to what, for the number `number` given to an
/// akt of `ty` created in `parent` (`akt.number.given`): `Ok(None)` for a
/// pattern that doesn't count; refused unless the pattern can give it.
// r[impl akt.number.given]
pub fn given(
    ty: &AktType,
    number: &str,
    parent: Option<&str>,
) -> Result<Option<(String, u64)>, String> {
    let read = ty.number().fits(number, parent).ok_or_else(|| {
        format!(
            "`{number}` isn't a number `{}` gives{}",
            ty.document().number,
            parent.map(|p| format!(" in {p}")).unwrap_or_default()
        )
    })?;
    Ok(read.map(|(scope, sequence)| (format!("number:{scope}"), sequence)))
}

/// Every number `ty`, which has no `{seq}`, can give an akt created at
/// `at` in `parent`, as a SQL `LIKE` pattern escaped with `\`.
#[must_use]
pub fn like(ty: &AktType, at: DateTime<Utc>, parent: Option<&str>) -> String {
    ty.number().like(&context(at, parent, &[]))
}

/// A free number for an akt of `ty`, which has no `{seq}`, created at `at`
/// in `parent`: drawn at random until `taken` doesn't hold it, then, for a
/// pattern of few digits, among the free ones. `None` when none is free.
// r[impl akt.number.random]
#[must_use]
pub fn draw_free(
    ty: &AktType,
    at: DateTime<Utc>,
    parent: Option<&str>,
    taken: &BTreeSet<String>,
    random: Random<'_>,
) -> Option<String> {
    let widths = ty.number().random_widths();
    let digits: u32 = widths.iter().map(|&w| wide(w)).sum();
    let number = |drawn: &[String]| render(ty, at, parent, drawn, 0);
    for _ in 0..DRAWS {
        let candidate = number(&draw(ty, random));
        if !taken.contains(&candidate) {
            return Some(candidate);
        }
    }
    if digits > LISTED {
        return None;
    }
    let free: Vec<String> = (0..10_u64.pow(digits))
        .map(|i| number(&digits_of(i, &widths)))
        .filter(|n| !taken.contains(n))
        .collect();
    if free.is_empty() {
        return None;
    }
    let at = usize::try_from(below(free.len() as u64, random)).unwrap_or(0);
    free.into_iter().nth(at)
}

/// The next number for an akt of `ty` created at `at` in `parent`, for a
/// store that keeps no counter (`cli.local.numbers`): with a `{seq}`, one
/// past the highest sequence of its counter among `existing` numbers; without
/// one, drawn free of them. `None` when no number is free.
// r[impl cli.local.numbers]
#[must_use]
pub fn next_from<'a>(
    ty: &AktType,
    at: DateTime<Utc>,
    parent: Option<&str>,
    existing: impl IntoIterator<Item = &'a str>,
    random: Random<'_>,
) -> Option<String> {
    if !ty.number().has_sequence() {
        let taken: BTreeSet<String> = existing.into_iter().map(str::to_owned).collect();
        return draw_free(ty, at, parent, &taken, random);
    }
    let drawn = draw(ty, random);
    let scope = ty.number().scope(&context(at, parent, &drawn));
    let highest = existing
        .into_iter()
        .filter_map(|number| ty.number().read(number))
        .filter(|(s, _)| *s == scope)
        .map(|(_, sequence)| sequence)
        .max()
        .unwrap_or(0);
    Some(render(ty, at, parent, &drawn, highest + 1))
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
            next_from(&task, at, Some("PLAN-001"), existing, &mut || 0).unwrap(),
            "PLAN-001-T08"
        );
        assert_eq!(
            next_from(&task, at, Some("PLAN-002"), existing, &mut || 0).unwrap(),
            "PLAN-002-T04"
        );
        assert_eq!(
            next_from(&task, at, Some("PLAN-003"), existing, &mut || 0).unwrap(),
            "PLAN-003-T01"
        );
        let capa = ty("CAPA-{year}-{seq:4}");
        assert_eq!(
            next_from(
                &capa,
                at,
                None,
                ["CAPA-2025-0009", "CAPA-2026-0002"],
                &mut || 0
            )
            .unwrap(),
            "CAPA-2026-0003"
        );
        assert_eq!(counter(&capa, at, None, &[]), "number:CAPA-2026-{seq:4}");
        assert_eq!(render(&capa, at, None, &[], 12), "CAPA-2026-0012");
    }

    /// Random numbers from a fixed list, then counting up.
    fn from(values: &[u64]) -> impl FnMut() -> u64 + '_ {
        let mut i = 0;
        move || {
            i += 1;
            values.get(i - 1).copied().unwrap_or(i as u64)
        }
    }

    // r[verify akt.number.random]
    #[test]
    fn draws_free_random_numbers() {
        let at = Utc.with_ymd_and_hms(2026, 11, 13, 0, 0, 0).unwrap();
        let qr = ty_flat("QR-{rand:2}");
        // 42 is taken, so the next draw, 07, is used: a digit a draw.
        let taken: BTreeSet<String> = ["QR-42".to_owned()].into();
        assert_eq!(
            draw_free(&qr, at, None, &taken, &mut from(&[4, 2, 0, 7])).as_deref(),
            Some("QR-07")
        );
        // Drawing keeps finding taken numbers: the one free is found.
        let all_but_one: BTreeSet<String> = (0..100)
            .filter(|&n| n != 63)
            .map(|n| format!("QR-{n:02}"))
            .collect();
        assert_eq!(
            draw_free(&qr, at, None, &all_but_one, &mut || 5).as_deref(),
            Some("QR-63")
        );
        let all: BTreeSet<String> = (0..100).map(|n| format!("QR-{n:02}")).collect();
        assert_eq!(draw_free(&qr, at, None, &all, &mut || 5), None);
        assert_eq!(like(&qr, at, None), "QR-__");
        assert_eq!(
            next_from(&qr, at, None, ["QR-07"], &mut from(&[0, 7, 0, 8])).as_deref(),
            Some("QR-08")
        );

        // Beside a sequence, the digits drawn scope its counter.
        let both = ty_flat("QR-{rand:2}-{seq:2}");
        let drawn = draw(&both, &mut from(&[3, 9]));
        assert_eq!(drawn, ["39"]);
        assert_eq!(counter(&both, at, None, &drawn), "number:QR-39-{seq:2}");
        assert_eq!(
            next_from(
                &both,
                at,
                None,
                ["QR-39-04", "QR-11-07"],
                &mut from(&[3, 9])
            )
            .as_deref(),
            Some("QR-39-05")
        );
    }

    fn ty_flat(pattern: &str) -> AktType {
        let document: AktTypeDocument = serde_json::from_value(json!({
            "key": "t", "name": "T", "number": pattern,
            "fields": [], "states": [{ "key": "open", "name": "Open", "initial": true }]
        }))
        .unwrap();
        validate(document).unwrap()
    }
}
