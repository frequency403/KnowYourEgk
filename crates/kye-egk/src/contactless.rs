//! Contactless access: EF.CardAccess, PACE with the CAN, secure channel.

use kye_pace::{CardAccess, PaceProtocol, Password};
use kye_sm::SecureChannel;
use kye_transport::CardTransport;

use crate::EgkError;
use crate::commands::select_mf;
use crate::objects::EF_CARD_ACCESS;
use crate::read::{MAX_CHUNK, read_sfi_with};

/// Selects the MF and reads EF.CardAccess (length from its outer TLV).
pub fn read_card_access<T: CardTransport + ?Sized>(t: &mut T) -> Result<Vec<u8>, EgkError> {
    let rsp = t.transmit(&select_mf())?;
    if !rsp.sw.is_ok() {
        return Err(EgkError::Status {
            command: "SELECT MF",
            sw: rsp.sw,
        });
    }
    read_sfi_with(t, EF_CARD_ACCESS.sfi, MAX_CHUNK, MAX_CHUNK, |h| {
        CardAccess::encoded_len(h).ok_or(EgkError::Format {
            file: "EF.CardAccess",
            reason: "does not start with a SET",
        })
    })
}

/// Runs PACE with `password` and returns a secure channel over `t`.
///
/// After this call every APDU, including SELECT, must go through the channel.
pub fn open_secure_channel<T: CardTransport>(
    mut t: T,
    password: &Password,
) -> Result<SecureChannel<T>, EgkError> {
    let raw = read_card_access(&mut t)?;
    let card_access = CardAccess::parse(&raw)?;
    let info = card_access
        .find_pace(PaceProtocol::ECDH_GM_AES_128)
        .ok_or(kye_pace::PaceError::Unsupported)?;
    let session = kye_pace::establish(&mut t, info, password)?;
    Ok(SecureChannel::new(
        t,
        &session.keys.k_enc,
        &session.keys.k_mac,
    ))
}
