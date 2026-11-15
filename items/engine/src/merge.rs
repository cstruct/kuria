//! Three-way merges of text: a change made from a base,
//! merged into a text that changed from the same base meanwhile, so that
//! both changes are kept.
//!
//! The texts are compared by lines, and where both changed the same or
//! neighbouring lines, by characters within them. Where both changed the
//! very same characters differently, the change being merged in wins,
//! being the later; where both inserted at the same place, both are kept,
//! the text's own first.

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
}
