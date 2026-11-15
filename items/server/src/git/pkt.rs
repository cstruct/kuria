//! git's pkt-lines: each a four-digit hexadecimal length, that counts
//! itself, and its data; `0000` a flush.

/// A flush packet.
pub(crate) const FLUSH: &[u8] = b"0000";

/// The most data one packet holds.
const MOST: usize = 65516;

/// `data` as one packet.
pub(crate) fn line(data: &[u8]) -> Vec<u8> {
    let mut out = format!("{:04x}", data.len() + 4).into_bytes();
    out.extend_from_slice(data);
    out
}

/// `data` on side band `band`, as packets small enough for
/// `side-band-64k`.
pub(crate) fn banded(band: u8, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in data.chunks(MOST - 1) {
        let mut packet = vec![band];
        packet.extend_from_slice(chunk);
        out.extend_from_slice(&line(&packet));
    }
    out
}

/// The packets at the start of `input` up to a flush, each one's data,
/// and what follows the flush.
pub(crate) fn read(input: &[u8]) -> Result<(Vec<&[u8]>, &[u8]), String> {
    let mut lines = Vec::new();
    let mut at = 0;
    loop {
        let head = input
            .get(at..at + 4)
            .ok_or_else(|| "the request ends before its flush".to_owned())?;
        let length = std::str::from_utf8(head)
            .ok()
            .and_then(|h| usize::from_str_radix(h, 16).ok())
            .ok_or_else(|| "a packet's length isn't hexadecimal".to_owned())?;
        if length == 0 {
            return Ok((lines, &input[at + 4..]));
        }
        if length < 4 {
            return Err(format!("a packet of length {length}"));
        }
        let data = input
            .get(at + 4..at + length)
            .ok_or_else(|| "a packet runs past the request".to_owned())?;
        lines.push(data);
        at += length;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_packets() {
        assert_eq!(line(b"a\n"), b"0006a\n");
        let mut input = line(b"one\n");
        input.extend_from_slice(&line(b"two"));
        input.extend_from_slice(FLUSH);
        input.extend_from_slice(b"PACK");
        let (lines, rest) = read(&input).unwrap();
        assert_eq!(lines, [&b"one\n"[..], &b"two"[..]]);
        assert_eq!(rest, b"PACK");
        assert!(read(b"0009ab").is_err());
        assert_eq!(banded(1, b"ok"), b"0007\x01ok");
    }
}
