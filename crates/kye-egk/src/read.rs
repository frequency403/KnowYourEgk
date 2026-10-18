//! Chunked transparent-file reads.

use kye_apdu::{Command, StatusWord};
use kye_transport::CardTransport;

use crate::EgkError;
use crate::commands::{MAX_OFFSET, read_binary, read_binary_sfi};

/// Largest Ne of a plain short READ BINARY.
pub const MAX_CHUNK: usize = 256;

/// Reads exactly `len` bytes of the EF with short file identifier `sfi`.
pub fn read_sfi_exact<T: CardTransport + ?Sized>(
    t: &mut T,
    sfi: u8,
    len: usize,
    chunk: usize,
) -> Result<Vec<u8>, EgkError> {
    read_sfi_with(t, sfi, chunk.min(len), chunk, |_| Ok(len))
}

/// Reads an EF whose total length is announced in its first bytes.
///
/// The first chunk (Ne = `first_ne`) is read via SFI, which also selects the
/// EF. `total` derives the file length from that chunk; the remainder is read
/// via 15-bit offsets on the now current EF in chunks of at most `chunk`
/// bytes. Nothing beyond the announced length is read.
pub fn read_sfi_with<T, F>(
    t: &mut T,
    sfi: u8,
    first_ne: usize,
    chunk: usize,
    total: F,
) -> Result<Vec<u8>, EgkError>
where
    T: CardTransport + ?Sized,
    F: FnOnce(&[u8]) -> Result<usize, EgkError>,
{
    let chunk = chunk.clamp(1, MAX_CHUNK);
    let first = read_chunk(t, read_binary_sfi(sfi, 0, first_ne.clamp(1, MAX_CHUNK)))?;
    let len = total(&first.0)?;
    let mut out = first.0;
    let mut eof = first.1;
    while out.len() < len {
        if eof {
            return Err(EgkError::ShortFile {
                got: out.len(),
                expected: len,
            });
        }
        let offset = u16::try_from(out.len())
            .ok()
            .filter(|o| *o <= MAX_OFFSET)
            .ok_or(EgkError::Format {
                file: "EF",
                reason: "offset exceeds 15 bit",
            })?;
        let (data, at_eof) = read_chunk(t, read_binary(offset, chunk.min(len - out.len())))?;
        out.extend_from_slice(&data);
        eof = at_eof;
    }
    out.truncate(len);
    Ok(out)
}

/// Sends one READ BINARY; returns the data and whether the card hit EOF.
fn read_chunk<T: CardTransport + ?Sized>(
    t: &mut T,
    cmd: Command,
) -> Result<(Vec<u8>, bool), EgkError> {
    let rsp = t.transmit(&cmd)?;
    let eof = rsp.sw == StatusWord::END_OF_FILE;
    if !rsp.sw.is_ok() && !eof {
        return Err(EgkError::Status {
            command: "READ BINARY",
            sw: rsp.sw,
        });
    }
    let eof = eof || rsp.data.is_empty();
    Ok((rsp.data, eof))
}

#[cfg(test)]
mod tests {
    use kye_transport::mock::MockTransport;

    use super::*;

    #[test]
    fn reads_in_chunks() {
        let mut first: Vec<u8> = (0u8..4).collect();
        first.extend_from_slice(&[0x90, 0x00]);
        let mut t = MockTransport::new([0x3B])
            .expect([0x00, 0xB0, 0x81, 0x00, 0x04], first)
            .expect([0x00, 0xB0, 0x00, 0x04, 0x02], [4, 5, 0x90, 0x00]);
        assert_eq!(
            read_sfi_exact(&mut t, 0x01, 6, 4).unwrap(),
            [0, 1, 2, 3, 4, 5]
        );
        assert!(t.is_done());
    }

    #[test]
    fn length_from_header() {
        // First chunk returns more than announced; nothing else is read.
        let mut t = MockTransport::new([0x3B]).expect(
            [0x00, 0xB0, 0x81, 0x00, 0x00],
            [0x00, 0x02, 0xAA, 0xBB, 0xCC, 0x62, 0x82],
        );
        let data = read_sfi_with(&mut t, 0x01, 256, 256, |h| {
            Ok(2 + usize::from(u16::from_be_bytes([h[0], h[1]])))
        })
        .unwrap();
        assert_eq!(data, [0x00, 0x02, 0xAA, 0xBB]);
        assert!(t.is_done());
    }

    #[test]
    fn short_file_and_errors() {
        let mut t =
            MockTransport::new([0x3B]).expect([0x00, 0xB0, 0x81, 0x00, 0x04], [0, 1, 0x62, 0x82]);
        assert!(matches!(
            read_sfi_exact(&mut t, 0x01, 4, 4),
            Err(EgkError::ShortFile {
                got: 2,
                expected: 4
            })
        ));

        let mut t = MockTransport::new([0x3B]).expect([0x00, 0xB0, 0x8C, 0x00, 0x19], [0x69, 0x82]);
        assert!(matches!(
            read_sfi_exact(&mut t, 0x0C, 25, 256),
            Err(EgkError::Status {
                sw: StatusWord::SECURITY_STATUS_NOT_SATISFIED,
                ..
            })
        ));
    }
}
