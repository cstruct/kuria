//! Three-way merges of text: a change made from a base,
//! merged into a text that changed from the same base meanwhile, so that
//! both changes are kept.
//!
//! The texts are compared by lines, and where both changed the same or
//! neighbouring lines, by characters within them. Where both changed the
//! very same characters differently, the change being merged in wins,
//! being the later; where both inserted at the same place, both are kept,
//! the text's own first.
//!
//! [`merge_marked`] instead reports where both changed the same text
//! differently, writing both between conflict markers, as a clone's pull
//! does (`clone.pull`).

use std::hash::Hash;
use std::time::{Duration, Instant};

use similar::{Algorithm, DiffOp, capture_diff_slices_deadline};

/// How long one diff may take before it settles for a coarser one.
const DIFF_TIMEOUT: Duration = Duration::from_millis(500);

/// `ours` with the change from `base` to `theirs` merged in, keeping what
/// `ours` changed from `base`.
#[must_use]
pub fn merge_text(base: &str, ours: &str, theirs: &str) -> String {
    if ours == base || ours == theirs {
        return theirs.to_owned();
    }
    if theirs == base {
        return ours.to_owned();
    }
    let lines =
        |text: &'_ str| -> Vec<String> { text.split_inclusive('\n').map(str::to_owned).collect() };
    let merged = merge(
        &lines(base),
        &lines(ours),
        &lines(theirs),
        true,
        &mut |base, ours, theirs| {
            let chars = |lines: &[String]| -> Vec<char> { lines.concat().chars().collect() };
            let merged: String = merge(
                &chars(base),
                &chars(ours),
                &chars(theirs),
                false,
                &mut |base, ours, theirs| {
                    if base.is_empty() {
                        [ours, theirs].concat()
                    } else {
                        theirs.to_vec()
                    }
                },
            )
            .into_iter()
            .collect();
            vec![merged]
        },
    );
    merged.concat()
}

/// The line a conflict starts with, before our side.
pub const OURS: &str = "<<<<<<< ours";
/// The line between our side of a conflict and theirs.
pub const BETWEEN: &str = "=======";
/// The line a conflict ends with, after their side.
pub const THEIRS: &str = ">>>>>>> kuria";

/// A three-way merge that marks conflicts: its text, and how many
/// conflicts it marked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marked {
    pub text: String,
    pub conflicts: usize,
}

/// `ours` with the change from `base` to `theirs` merged in, as
/// [`merge_text`] merges, except that where both changed the same text
/// differently both are kept, the lines they changed written between
/// [`OURS`], [`BETWEEN`] and [`THEIRS`] (`clone.pull`). Changes to
/// different lines merge; changes to the same lines merge by characters
/// when they change different characters, and conflict when they don't.
// r[impl clone.pull]
#[must_use]
pub fn merge_marked(base: &str, ours: &str, theirs: &str) -> Marked {
    if ours == base || ours == theirs {
        return Marked {
            text: theirs.to_owned(),
            conflicts: 0,
        };
    }
    if theirs == base {
        return Marked {
            text: ours.to_owned(),
            conflicts: 0,
        };
    }
    let lines =
        |text: &'_ str| -> Vec<String> { text.split_inclusive('\n').map(str::to_owned).collect() };
    let mut conflicts = 0;
    let merged = merge(
        &lines(base),
        &lines(ours),
        &lines(theirs),
        false,
        &mut |base, ours, theirs| {
            let chars = |lines: &[String]| -> Vec<char> { lines.concat().chars().collect() };
            let mut overlapping = false;
            let by_chars: String = merge(
                &chars(base),
                &chars(ours),
                &chars(theirs),
                false,
                &mut |_, _, theirs| {
                    overlapping = true;
                    theirs.to_vec()
                },
            )
            .into_iter()
            .collect();
            if !overlapping {
                return vec![by_chars];
            }
            conflicts += 1;
            let side = |lines: &[String]| {
                let mut text = lines.concat();
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text
            };
            vec![format!(
                "{OURS}\n{}{BETWEEN}\n{}{THEIRS}\n",
                side(ours),
                side(theirs)
            )]
        },
    );
    Marked {
        text: merged.concat(),
        conflicts,
    }
}

/// Whether `text` holds a conflict [`merge_marked`] marked: a line that
/// is [`OURS`] and one after it that is [`THEIRS`].
#[must_use]
pub fn has_conflict(text: &str) -> bool {
    let mut opened = false;
    for line in text.lines() {
        let line = line.trim_end();
        if line == OURS {
            opened = true;
        } else if opened && line == THEIRS {
            return true;
        }
    }
    false
}

/// One side's change of the base: `base[start..end]` replaced by `with`.
struct Hunk<'a, T> {
    start: usize,
    end: usize,
    with: &'a [T],
}

/// The changes from `base` to `new`, in order.
fn hunks<'a, T: Eq + Hash>(base: &[T], new: &'a [T]) -> Vec<Hunk<'a, T>> {
    let deadline = Instant::now().checked_add(DIFF_TIMEOUT);
    capture_diff_slices_deadline(Algorithm::Myers, base, new, deadline)
        .into_iter()
        .filter_map(|op| {
            let (start, len, at, new_len) = match op {
                DiffOp::Equal { .. } => return None,
                DiffOp::Delete {
                    old_index,
                    old_len,
                    new_index,
                } => (old_index, old_len, new_index, 0),
                DiffOp::Insert {
                    old_index,
                    new_index,
                    new_len,
                } => (old_index, 0, new_index, new_len),
                DiffOp::Replace {
                    old_index,
                    old_len,
                    new_index,
                    new_len,
                } => (old_index, old_len, new_index, new_len),
            };
            Some(Hunk {
                start,
                end: start + len,
                with: &new[at..at + new_len],
            })
        })
        .collect()
}

/// Whether a hunk over `start..end` meets a group over `from..to`: they
/// overlap, or, if `touching`, they are next to each other. An insertion
/// meets another at its place, or a range it is inside.
fn meets(start: usize, end: usize, from: usize, to: usize, touching: bool) -> bool {
    if touching {
        return start <= to && from <= end;
    }
    match (start == end, from == to) {
        (true, true) => start == from,
        (true, false) => from < start && start < to,
        (false, true) => start < from && from < end,
        (false, false) => start < to && from < end,
    }
}

/// `base[from..to]` with `hunks`, all within it, applied.
fn applied<T: Clone>(base: &[T], from: usize, to: usize, hunks: &[&Hunk<'_, T>]) -> Vec<T> {
    let mut out = Vec::new();
    let mut at = from;
    for hunk in hunks {
        out.extend_from_slice(&base[at..hunk.start]);
        out.extend_from_slice(hunk.with);
        at = hunk.end;
    }
    out.extend_from_slice(&base[at..to]);
    out
}

/// How a region both sides changed differently is resolved, from its base,
/// ours and theirs.
type Resolve<'a, T> = dyn FnMut(&[T], &[T], &[T]) -> Vec<T> + 'a;

/// `base` with the changes to `ours` and to `theirs` both made, each
/// region both changed resolved by `conflict(base, ours, theirs)` unless
/// they changed it alike.
fn merge<T: Clone + Eq + Hash>(
    base: &[T],
    ours: &[T],
    theirs: &[T],
    touching: bool,
    conflict: &mut Resolve<'_, T>,
) -> Vec<T> {
    let (a, b) = (hunks(base, ours), hunks(base, theirs));
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    let mut at = 0;
    while i < a.len() || j < b.len() {
        // The earlier first, an insertion before a range starting there.
        let first_ours = j >= b.len()
            || (i < a.len()
                && (a[i].start, a[i].end != a[i].start) <= (b[j].start, b[j].end != b[j].start));
        let first = if first_ours { &a[i] } else { &b[j] };
        let (from, mut to) = (first.start, first.end);
        let (mut mine, mut yours): (Vec<&Hunk<'_, T>>, Vec<&Hunk<'_, T>>) = (vec![], vec![]);
        if first_ours {
            mine.push(&a[i]);
            i += 1;
        } else {
            yours.push(&b[j]);
            j += 1;
        }
        loop {
            if i < a.len() && meets(a[i].start, a[i].end, from, to, touching) {
                to = to.max(a[i].end);
                mine.push(&a[i]);
                i += 1;
            } else if j < b.len() && meets(b[j].start, b[j].end, from, to, touching) {
                to = to.max(b[j].end);
                yours.push(&b[j]);
                j += 1;
            } else {
                break;
            }
        }
        out.extend_from_slice(&base[at..from]);
        let (kept, taken) = (
            applied(base, from, to, &mine),
            applied(base, from, to, &yours),
        );
        if yours.is_empty() {
            out.extend(kept);
        } else if mine.is_empty() || kept == taken {
            out.extend(taken);
        } else {
            out.extend(conflict(&base[from..to], &kept, &taken));
        }
        at = to;
    }
    out.extend_from_slice(&base[at..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_changes_to_different_lines() {
        let base = "# Title\n\nOne.\n\nTwo.\n\nThree.\n";
        let ours = "# Title\n\nOne, edited live.\n\nTwo.\n\nThree.\n";
        let theirs = "# Title\n\nOne.\n\nTwo.\n\nThree, pushed.\n";
        assert_eq!(
            merge_text(base, ours, theirs),
            "# Title\n\nOne, edited live.\n\nTwo.\n\nThree, pushed.\n"
        );
    }

    #[test]
    fn keeps_changes_to_one_line_by_characters() {
        let base = "The quick brown fox jumps.\n";
        let ours = "The quick red fox jumps.\n";
        let theirs = "The quick brown fox leaps.\n";
        assert_eq!(merge_text(base, ours, theirs), "The quick red fox leaps.\n");
        // Neighbouring lines, both changed.
        assert_eq!(merge_text("a\nb\n", "A\nb\n", "a\nB\n"), "A\nB\n",);
    }

    #[test]
    fn the_change_merged_in_wins_where_both_changed_alike_places() {
        assert_eq!(merge_text("cat\n", "dog\n", "cow\n"), "cow\n");
        // Both inserted at one place: both kept.
        assert_eq!(merge_text("ab", "aXb", "aYb"), "aXYb");
        // The same change on both sides, once.
        assert_eq!(merge_text("a\nb\n", "a\nc\n", "a\nc\n"), "a\nc\n");
    }

    #[test]
    fn takes_either_side_alone() {
        assert_eq!(merge_text("a\n", "a\n", "b\n"), "b\n");
        assert_eq!(merge_text("a\n", "b\n", "a\n"), "b\n");
        assert_eq!(merge_text("", "x\n", "y\n"), "x\ny\n");
        assert_eq!(
            merge_text("a\nb\nc\n", "a\nc\n", "a\nb\nc\nd\n"),
            "a\nc\nd\n"
        );
    }

    // r[verify clone.pull]
    #[test]
    fn marks_what_both_changed_alike() {
        let base = "# Title\n\nOne.\n\nTwo.\n";
        let ours = "# Title\n\nOne, mine.\n\nTwo.\n";
        let theirs = "# Title\n\nOne, theirs.\n\nTwo, theirs.\n";
        let merged = merge_marked(base, ours, theirs);
        assert_eq!(merged.conflicts, 1);
        assert_eq!(
            merged.text,
            "# Title\n\n<<<<<<< ours\nOne, mine.\n=======\nOne, theirs.\n>>>>>>> kuria\n\nTwo, theirs.\n"
        );
        assert!(has_conflict(&merged.text));
        // At the end, without a line break.
        let end = merge_marked("a\nb", "a\nc", "a\nd");
        assert_eq!(end.text, "a\n<<<<<<< ours\nc\n=======\nd\n>>>>>>> kuria\n");
    }

    // r[verify clone.pull]
    #[test]
    fn merges_without_markers_what_doesn_t_overlap() {
        let marked = |b: &str, o: &str, t: &str| {
            let m = merge_marked(b, o, t);
            assert_eq!(m.conflicts, 0, "{}", m.text);
            assert!(!has_conflict(&m.text));
            m.text
        };
        // Different lines.
        assert_eq!(marked("a\nb\nc\n", "A\nb\nc\n", "a\nb\nC\n"), "A\nb\nC\n");
        // One line, different words.
        assert_eq!(
            marked(
                "The quick brown fox jumps.\n",
                "The quick red fox jumps.\n",
                "The quick brown fox leaps.\n"
            ),
            "The quick red fox leaps.\n"
        );
        // The same change on both sides; one side alone.
        assert_eq!(marked("a\n", "b\n", "b\n"), "b\n");
        assert_eq!(marked("a\n", "a\n", "b\n"), "b\n");
        assert_eq!(marked("a\n", "b\n", "a\n"), "b\n");
        // Both inserting at one place conflicts.
        assert_eq!(merge_marked("a\n", "a\nx\n", "a\ny\n").conflicts, 1);
        assert!(!has_conflict("<<<<<<< ours\nno end\n"));
    }
}
