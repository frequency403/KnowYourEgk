//! Raw reads of EF.StatusVD, EF.PD and EF.VD.

use std::fmt;

use kye_transport::CardTransport;

use crate::EgkError;
use crate::commands::select_df_hca;
use crate::objects::{EF_PD, EF_STATUS_VD, EF_VD};
use crate::read::{MAX_CHUNK, read_sfi_exact, read_sfi_with};
use crate::status_vd::STATUS_VD_LEN;

/// Length of the EF.VD offset header.
pub const VD_HEADER_LEN: usize = 8;

/// Raw file contents, exactly as long as their headers announce.
///
/// Contains personal data: `Debug` only prints lengths.
#[derive(Clone, PartialEq, Eq)]
pub struct RawVsd {
    pub status_vd: Vec<u8>,
    pub pd: Vec<u8>,
    pub vd: Vec<u8>,
}

impl fmt::Debug for RawVsd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawVsd")
            .field("status_vd_len", &self.status_vd.len())
            .field("pd_len", &self.pd.len())
            .field("vd_len", &self.vd.len())
            .finish()
    }
}

/// Total length of EF.PD: `len(2, BE) || gzip(XML)`.
pub fn pd_total_len(header: &[u8]) -> Result<usize, EgkError> {
    match header {
        [hi, lo, ..] => Ok(2 + usize::from(u16::from_be_bytes([*hi, *lo]))),
        _ => Err(EgkError::Format {
            file: "EF.PD",
            reason: "missing length header",
        }),
    }
}

/// Offsets of the VD within EF.VD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VdOffsets {
    pub start_vd: u16,
    /// Offset of the last VD byte (inclusive).
    pub end_vd: u16,
}

impl VdOffsets {
    /// Parses `offStartVD || offEndVD || offStartGVD || offEndGVD`. The GVD
    /// copy is ignored.
    pub fn parse(header: &[u8]) -> Result<Self, EgkError> {
        let bad = |reason| EgkError::Format {
            file: "EF.VD",
            reason,
        };
        let h = header
            .get(..VD_HEADER_LEN)
            .ok_or(bad("missing offset header"))?;
        let start_vd = u16::from_be_bytes([h[0], h[1]]);
        let end_vd = u16::from_be_bytes([h[2], h[3]]);
        if usize::from(start_vd) < VD_HEADER_LEN || end_vd < start_vd {
            return Err(bad("invalid VD offsets"));
        }
        Ok(Self { start_vd, end_vd })
    }

    /// Bytes to read from offset 0 up to and including the last VD byte.
    pub fn total_len(&self) -> usize {
        usize::from(self.end_vd) + 1
    }
}

/// Selects DF.HCA and reads EF.StatusVD, EF.PD and EF.VD.
///
/// Works over a plain contact transport and over a secure channel alike.
/// `chunk` bounds the plaintext bytes per READ BINARY (max. 256).
pub fn read_raw_vsd<T: CardTransport + ?Sized>(
    t: &mut T,
    chunk: usize,
) -> Result<RawVsd, EgkError> {
    let chunk = chunk.clamp(1, MAX_CHUNK);
    let rsp = t.transmit(&select_df_hca())?;
    if !rsp.sw.is_ok() {
        return Err(EgkError::Status {
            command: "SELECT DF.HCA",
            sw: rsp.sw,
        });
    }
    let status_vd = read_sfi_exact(t, EF_STATUS_VD.sfi, STATUS_VD_LEN, chunk)?;
    let pd = read_sfi_with(t, EF_PD.sfi, chunk, chunk, pd_total_len)?;
    let vd = read_sfi_with(t, EF_VD.sfi, chunk, chunk, |h| {
        Ok(VdOffsets::parse(h)?.total_len())
    })?;
    Ok(RawVsd { status_vd, pd, vd })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pd_length() {
        assert_eq!(pd_total_len(&[0x01, 0x00, 0x1F]).unwrap(), 258);
        assert!(pd_total_len(&[0x01]).is_err());
    }

    #[test]
    fn vd_offsets() {
        let h = [0x00, 0x08, 0x01, 0x20, 0xFF, 0xFF, 0xFF, 0xFF];
        let o = VdOffsets::parse(&h).unwrap();
        assert_eq!(o.start_vd, 8);
        assert_eq!(o.total_len(), 0x121);
        assert!(VdOffsets::parse(&[0x00, 0x04, 0x01, 0x00, 0, 0, 0, 0]).is_err());
        assert!(VdOffsets::parse(&[0x00, 0x08, 0x00, 0x07, 0, 0, 0, 0]).is_err());
        assert!(VdOffsets::parse(&[0x00, 0x08]).is_err());
    }

    #[test]
    fn debug_hides_content() {
        let raw = RawVsd {
            status_vd: vec![1; 25],
            pd: vec![2; 3],
            vd: vec![3; 4],
        };
        assert_eq!(
            format!("{raw:?}"),
            "RawVsd { status_vd_len: 25, pd_len: 3, vd_len: 4 }"
        );
    }
}
