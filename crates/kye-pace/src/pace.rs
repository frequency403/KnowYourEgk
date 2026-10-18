//! PACE protocol run over a [`CardTransport`].

use bp256::r1::ProjectivePoint;
use kye_apdu::{StatusWord, tlv};
use kye_transport::CardTransport;

use crate::PaceError;
use crate::apdu::{ga_get_nonce, ga_response_objects, general_authenticate, mse_set_at_pace};
use crate::card_access::PaceInfo;
use crate::crypto::{
    EphemeralKey, Password, SessionKeys, auth_token, decode_point, decrypt_nonce, generic_mapping,
    nonce_scalar, tokens_equal, x_coordinate,
};
use crate::oid::{PaceProtocol, StandardizedDomainParameters};

const TAG_MAP_PCD: u32 = 0x81;
const TAG_MAP_PICC: u32 = 0x82;
const TAG_KA_PCD: u32 = 0x83;
const TAG_KA_PICC: u32 = 0x84;
const TAG_TOKEN_PCD: u32 = 0x85;
const TAG_TOKEN_PICC: u32 = 0x86;

/// Outcome of a successful PACE run.
#[derive(Debug)]
pub struct PaceSession {
    pub keys: SessionKeys,
    /// Compressed ephemeral public key of the chip, `x(PK2_PICC)` (ID_PICC).
    pub id_picc: [u8; 32],
}

/// Source of ephemeral keys; replaced by fixed keys in tests.
pub trait KeySource {
    fn next_key(&mut self, generator: &ProjectivePoint) -> Result<EphemeralKey, PaceError>;
}

/// Generates fresh random keys from the OS RNG.
pub struct OsKeys;

impl KeySource for OsKeys {
    fn next_key(&mut self, generator: &ProjectivePoint) -> Result<EphemeralKey, PaceError> {
        EphemeralKey::generate(generator)
    }
}

/// Runs PACE with random ephemeral keys.
pub fn establish<T: CardTransport + ?Sized>(
    t: &mut T,
    info: &PaceInfo,
    password: &Password,
) -> Result<PaceSession, PaceError> {
    establish_with(t, info, password, &mut OsKeys)
}

/// Runs PACE (MSE:Set AT + four General Authenticate steps) and verifies T_PICC.
pub fn establish_with<T: CardTransport + ?Sized, K: KeySource>(
    t: &mut T,
    info: &PaceInfo,
    password: &Password,
    keys: &mut K,
) -> Result<PaceSession, PaceError> {
    check_supported(info)?;

    // MSE:Set AT
    let rsp = t.transmit(&mse_set_at_pace(&info.oid, password.key_ref(), None))?;
    if !rsp.sw.is_ok() {
        return Err(status_error("MSE:Set AT", rsp.sw));
    }

    // Step 1: encrypted nonce.
    let rsp = t.transmit(&ga_get_nonce())?;
    let z = crate::apdu::parse_encrypted_nonce(&rsp)?;
    let k_pi = password.k_pi();
    let s = decrypt_nonce(&k_pi, &z)?;
    let s = nonce_scalar(&s)?;

    // Step 2: map nonce (generic mapping).
    let map_key = keys.next_key(&ProjectivePoint::GENERATOR)?;
    let rsp = t.transmit(&general_authenticate(
        &tlv::encode(TAG_MAP_PCD, &map_key.public_bytes()),
        false,
    ))?;
    let pk1_picc = decode_point(response_object("GA2 (map nonce)", &rsp, TAG_MAP_PICC)?)?;
    if pk1_picc == *map_key.public() {
        return Err(PaceError::InvalidPoint);
    }
    let h = map_key.agree(&pk1_picc)?;
    let g_tilde = generic_mapping(&s, &h)?;
    drop(map_key);

    // Step 3: key agreement on the mapped generator.
    let ka_key = keys.next_key(&g_tilde)?;
    let pk2_pcd = ka_key.public_bytes();
    let rsp = t.transmit(&general_authenticate(
        &tlv::encode(TAG_KA_PCD, &pk2_pcd),
        false,
    ))?;
    let pk2_picc_bytes = response_object("GA3 (key agreement)", &rsp, TAG_KA_PICC)?.to_vec();
    let pk2_picc = decode_point(&pk2_picc_bytes)?;
    if pk2_picc == *ka_key.public() {
        return Err(PaceError::InvalidPoint);
    }
    let kd = x_coordinate(&ka_key.agree(&pk2_picc)?);
    drop(ka_key);
    let session_keys = SessionKeys::derive(kd.as_slice());

    // Step 4: mutual authentication.
    let t_pcd = auth_token(&session_keys.k_mac, &info.oid, &pk2_picc_bytes);
    let rsp = t.transmit(&general_authenticate(
        &tlv::encode(TAG_TOKEN_PCD, &t_pcd),
        true,
    ))?;
    let t_picc = response_object("GA4 (mutual authentication)", &rsp, TAG_TOKEN_PICC)?;
    let expected = auth_token(&session_keys.k_mac, &info.oid, &pk2_pcd);
    if !tokens_equal(t_picc, &expected) {
        return Err(PaceError::TokenMismatch);
    }

    let mut id_picc = [0u8; 32];
    id_picc.copy_from_slice(&pk2_picc_bytes[1..33]);
    Ok(PaceSession {
        keys: session_keys,
        id_picc,
    })
}

fn check_supported(info: &PaceInfo) -> Result<(), PaceError> {
    let params_ok = info
        .domain_parameters()
        .is_none_or(|p| p == StandardizedDomainParameters::BrainpoolP256r1);
    if info.protocol() == Some(PaceProtocol::ECDH_GM_AES_128) && params_ok {
        Ok(())
    } else {
        Err(PaceError::Unsupported)
    }
}

fn status_error(step: &'static str, sw: StatusWord) -> PaceError {
    if sw.sw1() == 0x63 {
        PaceError::WrongPassword {
            retries: sw.retry_counter(),
        }
    } else {
        PaceError::Status { step, sw }
    }
}

fn response_object<'a>(
    step: &'static str,
    rsp: &'a kye_apdu::Response,
    tag: u32,
) -> Result<&'a [u8], PaceError> {
    if !rsp.sw.is_ok() {
        return Err(status_error(step, rsp.sw));
    }
    let objects = ga_response_objects(step, rsp)?;
    let obj = tlv::find(objects, tag)?.ok_or(PaceError::GaResponse("missing data object"))?;
    Ok(obj.value)
}
