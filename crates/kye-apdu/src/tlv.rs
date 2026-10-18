//! Minimal BER-TLV (ISO/IEC 8825-1) reader and writer.
//!
//! Tags are kept as their raw big-endian byte value (e.g. `0x7F49`), which is
//! how gemSpec_COS and ICAO 9303 write them.

use crate::ApduError;

/// A borrowed TLV object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tlv<'a> {
    pub tag: u32,
    pub value: &'a [u8],
}

impl Tlv<'_> {
    /// Whether the tag has the constructed bit set.
    pub fn is_constructed(&self) -> bool {
        let first = self.tag.to_be_bytes().into_iter().find(|b| *b != 0);
        first.is_some_and(|b| b & 0x20 != 0)
    }

    /// Iterates over the children of a constructed object.
    pub fn children(&self) -> TlvIter<'_> {
        TlvIter::new(self.value)
    }
}

/// Parses one TLV object from the front of `input` and returns the rest.
pub fn parse_one(input: &[u8]) -> Result<(Tlv<'_>, &[u8]), ApduError> {
    let (tag, rest) = parse_tag(input)?;
    let (len, rest) = parse_length(rest)?;
    if rest.len() < len {
        return Err(ApduError::Tlv("value exceeds input"));
    }
    let (value, rest) = rest.split_at(len);
    Ok((Tlv { tag, value }, rest))
}

fn parse_tag(input: &[u8]) -> Result<(u32, &[u8]), ApduError> {
    let (&first, mut rest) = input.split_first().ok_or(ApduError::Tlv("empty input"))?;
    let mut tag = u32::from(first);
    if first & 0x1F == 0x1F {
        loop {
            let (&b, r) = rest.split_first().ok_or(ApduError::Tlv("truncated tag"))?;
            rest = r;
            if tag > 0x00FF_FFFF {
                return Err(ApduError::Tlv("tag longer than 4 bytes"));
            }
            tag = (tag << 8) | u32::from(b);
            if b & 0x80 == 0 {
                break;
            }
        }
    }
    Ok((tag, rest))
}

fn parse_length(input: &[u8]) -> Result<(usize, &[u8]), ApduError> {
    let (&first, rest) = input
        .split_first()
        .ok_or(ApduError::Tlv("missing length"))?;
    if first < 0x80 {
        return Ok((usize::from(first), rest));
    }
    let n = usize::from(first & 0x7F);
    if n == 0 {
        return Err(ApduError::Tlv("indefinite length not supported"));
    }
    if n > 4 || rest.len() < n {
        return Err(ApduError::Tlv("bad length field"));
    }
    let (bytes, rest) = rest.split_at(n);
    let len = bytes
        .iter()
        .fold(0usize, |acc, b| (acc << 8) | usize::from(*b));
    Ok((len, rest))
}

/// Iterator over consecutive TLV objects. Yields an error once and then stops.
pub struct TlvIter<'a> {
    rest: &'a [u8],
}

impl<'a> TlvIter<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self { rest: input }
    }
}

impl<'a> Iterator for TlvIter<'a> {
    type Item = Result<Tlv<'a>, ApduError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            return None;
        }
        match parse_one(self.rest) {
            Ok((tlv, rest)) => {
                self.rest = rest;
                Some(Ok(tlv))
            }
            Err(e) => {
                self.rest = &[];
                Some(Err(e))
            }
        }
    }
}

/// Finds the first object with `tag` among the siblings in `input`.
pub fn find(input: &[u8], tag: u32) -> Result<Option<Tlv<'_>>, ApduError> {
    for tlv in TlvIter::new(input) {
        let tlv = tlv?;
        if tlv.tag == tag {
            return Ok(Some(tlv));
        }
    }
    Ok(None)
}

/// Appends the DER length encoding of `len` to `out`.
pub fn write_length(out: &mut Vec<u8>, len: usize) {
    match len {
        0..=0x7F => out.push(len as u8),
        0x80..=0xFF => out.extend_from_slice(&[0x81, len as u8]),
        0x100..=0xFFFF => {
            out.push(0x82);
            out.extend_from_slice(&(len as u16).to_be_bytes());
        }
        _ => {
            out.push(0x84);
            out.extend_from_slice(&(len as u32).to_be_bytes());
        }
    }
}

/// Appends `tag || len || value` to `out`.
pub fn write(out: &mut Vec<u8>, tag: u32, value: &[u8]) {
    let tag_bytes = tag.to_be_bytes();
    let skip = tag_bytes.iter().take(3).take_while(|b| **b == 0).count();
    out.extend_from_slice(&tag_bytes[skip..]);
    write_length(out, value.len());
    out.extend_from_slice(value);
}

/// Returns `tag || len || value` as a new vector.
pub fn encode(tag: u32, value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + 6);
    write(&mut out, tag, value);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_multibyte_tag() {
        let enc = encode(0x7F49, &[0x86, 0x01, 0xAA]);
        assert_eq!(enc, [0x7F, 0x49, 0x03, 0x86, 0x01, 0xAA]);
        let (tlv, rest) = parse_one(&enc).unwrap();
        assert!(rest.is_empty());
        assert_eq!(tlv.tag, 0x7F49);
        assert!(tlv.is_constructed());
        let child = tlv.children().next().unwrap().unwrap();
        assert_eq!(
            child,
            Tlv {
                tag: 0x86,
                value: &[0xAA]
            }
        );
        assert!(!child.is_constructed());
    }

    #[test]
    fn long_lengths() {
        let value = vec![0u8; 300];
        let enc = encode(0x53, &value);
        assert_eq!(&enc[..4], &[0x53, 0x82, 0x01, 0x2C]);
        let (tlv, _) = parse_one(&enc).unwrap();
        assert_eq!(tlv.value.len(), 300);

        let enc = encode(0x04, &[0u8; 0x81]);
        assert_eq!(&enc[..3], &[0x04, 0x81, 0x81]);
    }

    #[test]
    fn zero_tag_byte_kept() {
        // A 1-byte tag 0x00 must still be emitted.
        assert_eq!(encode(0x00, &[]), [0x00, 0x00]);
    }

    #[test]
    fn errors() {
        assert!(parse_one(&[0x80, 0x05, 0x01]).is_err());
        assert!(parse_one(&[0x7F]).is_err());
        assert!(parse_one(&[0x30, 0x80]).is_err());
        let mut it = TlvIter::new(&[0x80, 0x01, 0x00, 0x81, 0x09]);
        assert!(it.next().unwrap().is_ok());
        assert!(it.next().unwrap().is_err());
        assert!(it.next().is_none());
    }

    #[test]
    fn find_sibling() {
        let data = [0x80, 0x01, 0x01, 0x82, 0x02, 0xAB, 0xCD];
        assert_eq!(find(&data, 0x82).unwrap().unwrap().value, [0xAB, 0xCD]);
        assert!(find(&data, 0x84).unwrap().is_none());
    }
}
