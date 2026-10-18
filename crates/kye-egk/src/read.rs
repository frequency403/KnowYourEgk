//! Chunked transparent-file reads.

use kye_apdu::{Command, StatusWord};
use kye_transport::CardTransport;

use crate::EgkError;
use crate::commands::{MAX_OFFSET, read_binary, read_binary_sfi};

/// Reads exactly `len` bytes of the EF with short file identifier `sfi`.
///
/// The first chunk is read via SFI (which selects the EF), every further
/// chunk via 15-bit offset on the now current EF. `chunk` bounds Ne per
/// command and must be in `1..=256` for plain short APDUs.
pub fn read_sfi_exact<T: CardTransport + ?Sized>(
    t: &mut T,
    sfi: u8,
    len: usize,
    chunk: usize,
) -> Result<Vec<u8>, EgkError> {
    let chunk = chunk.clamp(1, 256);
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let ne = chunk.min(len - out.len());
        let cmd = next_chunk_command(sfi, out.len(), ne)?;
        let rsp = t.transmit(&cmd)?;
        let eof = rsp.sw == StatusWord::END_OF_FILE;
        if !rsp.sw.is_ok() && !eof {
            return Err(EgkError::Status {
                command: "READ BINARY",
                sw: rsp.sw,
            });
        }
        if rsp.data.is_empty() || (eof && out.len() + rsp.data.len() < len) {
            out.extend_from_slice(&rsp.data);
            return Err(EgkError::ShortFile {
                got: out.len(),
                expected: len,
            });
        }
        let take = rsp.data.len().min(len - out.len());
        out.extend_from_slice(&rsp.data[..take]);
    }
    Ok(out)
}

fn next_chunk_command(sfi: u8, offset: usize, ne: usize) -> Result<Command, EgkError> {
    if offset == 0 {
        return Ok(read_binary_sfi(sfi, 0, ne));
    }
    let offset = u16::try_from(offset)
        .ok()
        .filter(|o| *o <= MAX_OFFSET)
        .ok_or(EgkError::Format {
            file: "EF",
            reason: "offset exceeds 15 bit",
        })?;
    Ok(read_binary(offset, ne))
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
