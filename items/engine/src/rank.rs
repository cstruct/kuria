//! Rank keys: strings whose byte order is a tenant's priority order, with a
//! key between any two.
//!
//! The scheme is fractional indexing with an integer part, as in the widely
//! used `fractional-indexing` library: a key is an integer part, whose first
//! character says how many digits follow (`a` one, `b` two, … and `Z`, `Y`, …
//! the same below zero), then an optional fraction without trailing zeros.
//! Appending increments the integer part, so a list built by appending keeps
//! keys of a few characters; inserting between two keys extends the
//! fraction only as far as it must.
//!
//! Digits are `0-9A-Za-z`, in ASCII order, so keys must be compared byte by
//! byte (in Postgres, `COLLATE "C"`).

const DIGITS: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const ZERO: u8 = b'0';
const LAST: u8 = b'z';

/// Why no key could be made.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RankError {
    #[error("`{0}` is not a rank key")]
    Invalid(String),
    #[error("`{0}` doesn't come before `{1}`")]
    Order(String, String),
    #[error("the rank keys are exhausted at this end")]
    Exhausted,
}

fn digit(value: u8) -> Result<usize, RankError> {
    DIGITS
        .iter()
        .position(|d| *d == value)
        .ok_or_else(|| RankError::Invalid(char::from(value).to_string()))
}

fn integer_length(head: u8) -> Option<usize> {
    match head {
        b'a'..=b'z' => Some(usize::from(head - b'a') + 2),
        b'A'..=b'Z' => Some(usize::from(b'Z' - head) + 2),
        _ => None,
    }
}

/// The smallest integer part, below which nothing can go.
fn smallest_integer() -> Vec<u8> {
    let mut key = vec![b'A'];
    key.extend(std::iter::repeat_n(ZERO, 26));
    key
}

/// Splits a key into its integer part and fraction, checking it.
fn split(key: &[u8]) -> Result<(&[u8], &[u8]), RankError> {
    let invalid = || RankError::Invalid(String::from_utf8_lossy(key).into_owned());
    let length = key
        .first()
        .and_then(|h| integer_length(*h))
        .ok_or_else(invalid)?;
    if length > key.len() || key == smallest_integer() {
        return Err(invalid());
    }
    for byte in &key[1..] {
        digit(*byte).map_err(|_| invalid())?;
    }
    let (integer, fraction) = key.split_at(length);
    if fraction.last() == Some(&ZERO) {
        return Err(invalid());
    }
    Ok((integer, fraction))
}

/// A fraction strictly between `a` and `b` (or above `a` when `b` is
/// `None`), both without trailing zeros, `a` possibly empty.
fn midpoint(a: &[u8], b: Option<&[u8]>) -> Vec<u8> {
    if let Some(b) = b {
        let mut n = 0;
        while a.get(n).copied().unwrap_or(ZERO) == b[n] {
            n += 1;
        }
        if n > 0 {
            let mut key = b[..n].to_vec();
            key.extend(midpoint(a.get(n..).unwrap_or_default(), Some(&b[n..])));
            return key;
        }
    }
    let digit_a = a.first().map_or(0, |d| digit(*d).unwrap_or(0));
    let digit_b = b.map_or(DIGITS.len(), |b| digit(b[0]).unwrap_or(0));
    if digit_b - digit_a > 1 {
        // Rounds half up, as the original scheme does.
        return vec![DIGITS[(digit_a + digit_b).div_ceil(2)]];
    }
    match b {
        Some(b) if b.len() > 1 => vec![b[0]],
        _ => {
            let mut key = vec![DIGITS[digit_a]];
            key.extend(midpoint(a.get(1..).unwrap_or_default(), None));
            key
        }
    }
}

/// The next integer part, or `None` past the largest.
fn increment(integer: &[u8]) -> Option<Vec<u8>> {
    let (head, digits) = (integer[0], &integer[1..]);
    let mut digits = digits.to_vec();
    let mut carry = true;
    for d in digits.iter_mut().rev() {
        if !carry {
            break;
        }
        let next = digit(*d).unwrap_or(0) + 1;
        if next == DIGITS.len() {
            *d = ZERO;
        } else {
            *d = DIGITS[next];
            carry = false;
        }
    }
    if !carry {
        let mut key = vec![head];
        key.extend(digits);
        return Some(key);
    }
    match head {
        b'Z' => Some(vec![b'a', ZERO]),
        b'z' => None,
        _ => {
            let head = head + 1;
            if head > b'a' {
                digits.push(ZERO);
            } else {
                digits.pop();
            }
            let mut key = vec![head];
            key.extend(digits);
            Some(key)
        }
    }
}

/// The previous integer part, or `None` below the smallest.
fn decrement(integer: &[u8]) -> Option<Vec<u8>> {
    let (head, digits) = (integer[0], &integer[1..]);
    let mut digits = digits.to_vec();
    let mut borrow = true;
    for d in digits.iter_mut().rev() {
        if !borrow {
            break;
        }
        let value = digit(*d).unwrap_or(0);
        if value == 0 {
            *d = LAST;
        } else {
            *d = DIGITS[value - 1];
            borrow = false;
        }
    }
    if !borrow {
        let mut key = vec![head];
        key.extend(digits);
        return Some(key);
    }
    match head {
        b'a' => Some(vec![b'Z', LAST]),
        b'A' => None,
        _ => {
            let head = head - 1;
            if head < b'Z' {
                digits.push(LAST);
            } else {
                digits.pop();
            }
            let mut key = vec![head];
            key.extend(digits);
            Some(key)
        }
    }
}

/// The rank an akt at `own` gets to sit between the ranks `before` and
/// `after`, the neighbours of where it moves, or `None` if it already sits
/// right there and nothing moves.
///
/// # Errors
///
/// When `before` and `after` aren't ranks in order.
// r[impl order.move]
pub fn placement(
    own: &str,
    before: Option<&str>,
    after: Option<&str>,
) -> Result<Option<String>, RankError> {
    if before == Some(own) || after == Some(own) {
        return Ok(None);
    }
    rank_between(before, after).map(Some)
}

/// A key strictly between `before` and `after`, where `None` means the start
/// or the end of the order.
///
/// # Panics
///
/// Never: keys are built from ASCII digits only.
// r[impl order.move]
pub fn rank_between(before: Option<&str>, after: Option<&str>) -> Result<String, RankError> {
    let a = before.map(str::as_bytes);
    let b = after.map(str::as_bytes);
    let a_parts = a.map(split).transpose()?;
    let b_parts = b.map(split).transpose()?;
    if let (Some(a), Some(b)) = (a, b)
        && a >= b
    {
        return Err(RankError::Order(
            String::from_utf8_lossy(a).into_owned(),
            String::from_utf8_lossy(b).into_owned(),
        ));
    }
    let key = match (a_parts, b_parts, b) {
        (None, None, _) => vec![b'a', ZERO],
        (None, Some((integer, fraction)), Some(b)) => {
            if integer == smallest_integer() {
                let mut key = integer.to_vec();
                key.extend(midpoint(&[], Some(fraction)));
                key
            } else if integer < b {
                integer.to_vec()
            } else {
                decrement(integer).ok_or(RankError::Exhausted)?
            }
        }
        (Some((integer, fraction)), None, _) => match increment(integer) {
            Some(next) => next,
            None => {
                let mut key = integer.to_vec();
                key.extend(midpoint(fraction, None));
                key
            }
        },
        (Some((ia, fa)), Some((ib, fb)), Some(b)) => {
            if ia == ib {
                let mut key = ia.to_vec();
                key.extend(midpoint(fa, Some(fb)));
                key
            } else {
                let next = increment(ia).ok_or(RankError::Exhausted)?;
                if next.as_slice() < b {
                    next
                } else {
                    let mut key = ia.to_vec();
                    key.extend(midpoint(fa, None));
                    key
                }
            }
        }
        (_, Some(_), None) => unreachable!("b_parts comes from b"),
    };
    Ok(String::from_utf8(key).expect("keys are ASCII"))
}

/// The key of the `n`th akt appended to an empty order, counting from 0:
/// `a0` … `az`, `b00` … `bzz`, `c000` and so on, which is what appending
/// with [`rank_between`] gives.
///
/// # Panics
///
/// Never for an `n` below 62²⁶, far beyond any order's length.
#[must_use]
pub fn nth_appended(mut n: u64) -> String {
    let mut length: u32 = 1;
    let base = DIGITS.len() as u64;
    while n >= base.pow(length) {
        n -= base.pow(length);
        length += 1;
    }
    let mut digits = vec![ZERO; length as usize];
    for slot in digits.iter_mut().rev() {
        *slot = DIGITS[usize::try_from(n % base).expect("a digit")];
        n /= base;
    }
    let head = b'a' + u8::try_from(length - 1).expect("at most 26 digits");
    let mut key = vec![head];
    key.extend(digits);
    String::from_utf8(key).expect("keys are ASCII")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn between(a: Option<&str>, b: Option<&str>) -> String {
        let key = rank_between(a, b).unwrap();
        if let Some(a) = a {
            assert!(a < key.as_str(), "{a} < {key}");
        }
        if let Some(b) = b {
            assert!(key.as_str() < b, "{key} < {b}");
        }
        key
    }

    // r[verify order.move]
    #[test]
    fn matches_the_reference_scheme() {
        assert_eq!(between(None, None), "a0");
        assert_eq!(between(Some("a0"), None), "a1");
        assert_eq!(between(Some("a1"), None), "a2");
        assert_eq!(between(None, Some("a0")), "Zz");
        assert_eq!(between(Some("a0"), Some("a1")), "a0V");
        assert_eq!(between(Some("a1"), Some("a2")), "a1V");
        assert_eq!(between(Some("a0V"), Some("a1")), "a0l");
        assert_eq!(between(Some("Zz"), Some("a0")), "ZzV");
        assert_eq!(between(Some("Zz"), Some("a1")), "a0");
        assert_eq!(between(Some("az"), None), "b00");
        assert_eq!(
            between(Some("zzzzzzzzzzzzzzzzzzzzzzzzzzz"), None),
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzV"
        );
        assert_eq!(between(Some("b125"), Some("b129")), "b127");
        assert_eq!(between(Some("a0"), Some("a0V")), "a0G");
    }

    // r[verify order.move]
    #[test]
    fn refuses_bad_keys_and_orders() {
        assert!(matches!(
            rank_between(Some("a1"), Some("a0")),
            Err(RankError::Order(..))
        ));
        assert!(matches!(
            rank_between(Some("a0"), Some("a0")),
            Err(RankError::Order(..))
        ));
        for key in ["", "a", "a00", "0", "a0!"] {
            assert!(
                matches!(rank_between(Some(key), None), Err(RankError::Invalid(_))),
                "{key}"
            );
        }
    }

    // r[verify order.move]
    #[test]
    fn appending_keeps_keys_short() {
        let mut key: Option<String> = None;
        for n in 0..5_000 {
            let next = between(key.as_deref(), None);
            assert_eq!(next, nth_appended(n));
            key = Some(next);
        }
        assert_eq!(key.unwrap().len(), 4);
    }

    // r[verify order.move]
    #[test]
    fn always_finds_a_key_between_neighbours() {
        // Repeatedly insert right after the first key, the worst case for
        // fraction growth, and check order holds throughout.
        let mut keys = vec![between(None, None)];
        keys.push(between(Some(&keys[0]), None));
        for _ in 0..200 {
            let key = between(Some(&keys[0]), Some(&keys[1]));
            keys.insert(1, key);
        }
        for pair in keys.windows(2) {
            assert!(pair[0] < pair[1]);
        }
        // And at the very start, before the first key, many times.
        let mut first = keys[0].clone();
        for _ in 0..200 {
            first = between(None, Some(&first));
        }
    }
}
