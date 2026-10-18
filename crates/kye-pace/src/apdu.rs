//! Command builders and response parsers for MSE:Set AT and General Authenticate.

use kye_apdu::{Command, MAX_NE_SHORT, Response, tlv};

use crate::PaceError;
use crate::oid::Oid;

/// Password used for PACE (key reference in MSE:Set AT, tag 83).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PasswordRef {
    Mrz = 0x01,
    Can = 0x02,
    Pin = 0x03,
    Puk = 0x04,
}

const TAG_DYN_AUTH_DATA: u32 = 0x7C;
const TAG_ENCRYPTED_NONCE: u32 = 0x80;

/// `MSE:Set AT` selecting PACE with the given protocol and password
/// (gemSpec_COS CosT_a40): `00 22 C1 A4 Lc [80 OID] [83 keyRef] ([84 paramId])`.
pub fn mse_set_at_pace(protocol: &Oid, password: PasswordRef, parameter_id: Option<u8>) -> Command {
    let mut data = tlv::encode(0x80, protocol.as_bytes());
    tlv::write(&mut data, 0x83, &[password as u8]);
    if let Some(id) = parameter_id {
        tlv::write(&mut data, 0x84, &[id]);
    }
    Command::new(0x00, 0x22, 0xC1, 0xA4).with_data(data)
}

/// `GENERAL AUTHENTICATE` carrying the given dynamic authentication data
/// objects inside a `7C` template. All steps except the last are chained.
pub fn general_authenticate(objects: &[u8], last: bool) -> Command {
    let cla = if last { 0x00 } else { 0x10 };
    Command::new(cla, 0x86, 0x00, 0x00)
        .with_data(tlv::encode(TAG_DYN_AUTH_DATA, objects))
        .with_ne(MAX_NE_SHORT)
}

/// Step 1: request the encrypted nonce (`10 86 00 00 02 7C 00 00`).
pub fn ga_get_nonce() -> Command {
    general_authenticate(&[], false)
}

/// Extracts the `7C` content of a successful General Authenticate response.
pub fn ga_response_objects<'a>(
    step: &'static str,
    rsp: &'a Response,
) -> Result<&'a [u8], PaceError> {
    if !rsp.sw.is_ok() {
        return Err(PaceError::Status { step, sw: rsp.sw });
    }
    let (outer, rest) = tlv::parse_one(&rsp.data)?;
    if outer.tag != TAG_DYN_AUTH_DATA || !rest.is_empty() {
        return Err(PaceError::GaResponse("expected a single 7C template"));
    }
    Ok(outer.value)
}

/// Parses the step 1 response `7C 12 80 10 <z>` and returns the encrypted nonce z.
pub fn parse_encrypted_nonce(rsp: &Response) -> Result<Vec<u8>, PaceError> {
    let objects = ga_response_objects("GA1 (encrypted nonce)", rsp)?;
    let nonce = tlv::find(objects, TAG_ENCRYPTED_NONCE)?
        .ok_or(PaceError::GaResponse("missing encrypted nonce (80)"))?;
    if nonce.value.is_empty() || !nonce.value.len().is_multiple_of(16) {
        return Err(PaceError::GaResponse(
            "encrypted nonce is not a multiple of the AES block size",
        ));
    }
    Ok(nonce.value.to_vec())
}

#[cfg(test)]
mod tests {
    use kye_apdu::StatusWord;

    use super::*;

    fn egk_oid() -> Oid {
        Oid(vec![
            0x04, 0x00, 0x7F, 0x00, 0x07, 0x02, 0x02, 0x04, 0x02, 0x02,
        ])
    }

    #[test]
    fn mse_matches_brief() {
        let cmd = mse_set_at_pace(&egk_oid(), PasswordRef::Can, None);
        assert_eq!(
            cmd.to_bytes().unwrap(),
            [
                0x00, 0x22, 0xC1, 0xA4, 0x0F, 0x80, 0x0A, 0x04, 0x00, 0x7F, 0x00, 0x07, 0x02, 0x02,
                0x04, 0x02, 0x02, 0x83, 0x01, 0x02
            ]
        );
    }

    #[test]
    fn ga1_matches_brief() {
        assert_eq!(
            ga_get_nonce().to_bytes().unwrap(),
            [0x10, 0x86, 0x00, 0x00, 0x02, 0x7C, 0x00, 0x00]
        );
    }

    #[test]
    fn parse_nonce() {
        let mut data = vec![0x7C, 0x12, 0x80, 0x10];
        data.extend(1u8..=16);
        let rsp = Response::new(data, StatusWord::OK);
        assert_eq!(
            parse_encrypted_nonce(&rsp).unwrap(),
            (1u8..=16).collect::<Vec<_>>()
        );

        let rsp = Response::new(vec![], StatusWord(0x6A80));
        assert!(matches!(
            parse_encrypted_nonce(&rsp),
            Err(PaceError::Status { .. })
        ));

        let rsp = Response::new(vec![0x7C, 0x03, 0x80, 0x01, 0x00], StatusWord::OK);
        assert!(matches!(
            parse_encrypted_nonce(&rsp),
            Err(PaceError::GaResponse(_))
        ));
    }
}
