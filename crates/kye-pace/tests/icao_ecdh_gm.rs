//! ICAO Doc 9303 Part 11, Appendix G.1: worked example for PACE
//! ECDH-GM, brainpoolP256r1, AES-128 (password: MRZ instead of CAN).

#![allow(clippy::unwrap_used)]

use std::collections::VecDeque;

use bp256::elliptic_curve::PrimeField;
use bp256::r1::{NonZeroScalar, ProjectivePoint};
use kye_pace::crypto::{
    EphemeralKey, Password, SessionKeys, auth_token, decode_point, decrypt_nonce, encode_point,
    generic_mapping, kdf, nonce_scalar, x_coordinate,
};
use kye_pace::{KeySource, Oid, PaceError, PaceInfo, establish_with};
use kye_transport::mock::MockTransport;

const MRZ: &str = "T22000129364081251010318";
const K_PI: &str = "89DED1B26624EC1E634C1989302849DD";
const Z: &str = "95A3A016522EE98D01E76CB6B98B42C3";
const S: &str = "3F00C4D39D153F2B2A214A078D899B22";

const SK1_PCD: &str = "7F4EF07B9EA82FD78AD689B38D0BC78CF21F249D953BC46F4C6E19259C010F99";
const PK1_PCD: &str = "04\
    7ACF3EFC982EC45565A4B155129EFBC74650DCBFA6362D896FC70262E0C2CC5E\
    544552DCB6725218799115B55C9BAA6D9F6BC3A9618E70C25AF71777A9C4922D";
const PK1_PICC: &str = "04\
    824FBA91C9CBE26BEF53A0EBE7342A3BF178CEA9F45DE0B70AA601651FBA3F57\
    30D8C879AAA9C9F73991E61B58F4D52EB87A0A0C709A49DC63719363CCD13C54";
const H: &str = "04\
    60332EF2450B5D247EF6D3868397D398852ED6E8CAF6FFEEF6BF85CA57057FD5\
    0840CA7415BAF3E43BD414D35AA4608B93A2CAF3A4E3EA4E82C9C13D03EB7181";
const G_TILDE: &str = "04\
    8CED63C91426D4F0EB1435E7CB1D74A46723A0AF21C89634F65A9AE87A9265E2\
    8C879506743F8611AC33645C5B985C80B5F09A0B83407C1B6A4D857AE76FE522";

const SK2_PCD: &str = "A73FB703AC1436A18E0CFA5ABB3F7BEC7A070E7A6788486BEE230C4A22762595";
const PK2_PCD: &str = "04\
    2DB7A64C0355044EC9DF190514C625CBA2CEA48754887122F3A5EF0D5EDD301C\
    3556F3B3B186DF10B857B58F6A7EB80F20BA5DC7BE1D43D9BF850149FBB36462";
const PK2_PICC: &str = "04\
    9E880F842905B8B3181F7AF7CAA9F0EFB743847F44A306D2D28C1D9EC65DF6DB\
    7764B22277A2EDDC3C265A9F018F9CB852E111B768B326904B59A0193776F094";
const KD: &str = "28768D20701247DAE81804C9E780EDE582A9996DB4A315020B2733197DB84925";
const K_ENC: &str = "F5F0E35C0D7161EE6724EE513A0D9A7F";
const K_MAC: &str = "FE251C7858B356B24514B3BD5F4297D1";
const T_PCD: &str = "C2B0BD78D94BA866";
const T_PICC: &str = "3ABB9674BCE93C08";

const OID: [u8; 10] = [0x04, 0x00, 0x7F, 0x00, 0x07, 0x02, 0x02, 0x04, 0x02, 0x02];

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn key16(s: &str) -> [u8; 16] {
    hex(s).try_into().unwrap()
}

fn scalar(s: &str) -> NonZeroScalar {
    let bytes: [u8; 32] = hex(s).try_into().unwrap();
    let repr = <bp256::Scalar as PrimeField>::Repr::from(bytes);
    NonZeroScalar::from_repr(repr).unwrap()
}

fn mrz() -> Password {
    Password::Mrz(MRZ.to_owned())
}

#[test]
fn k_pi_from_mrz() {
    assert_eq!(*mrz().k_pi(), key16(K_PI));
}

#[test]
fn nonce_decryption() {
    let s = decrypt_nonce(&key16(K_PI), &hex(Z)).unwrap();
    assert_eq!(s.as_slice(), hex(S));
}

#[test]
fn generic_mapping_and_key_agreement() {
    let k1 = EphemeralKey::from_secret(scalar(SK1_PCD), &ProjectivePoint::GENERATOR);
    assert_eq!(k1.public_bytes().to_vec(), hex(PK1_PCD));

    let h = k1.agree(&decode_point(&hex(PK1_PICC)).unwrap()).unwrap();
    assert_eq!(encode_point(&h).to_vec(), hex(H));

    let s = nonce_scalar(&hex(S)).unwrap();
    let g = generic_mapping(&s, &h).unwrap();
    assert_eq!(encode_point(&g.to_affine()).to_vec(), hex(G_TILDE));

    let k2 = EphemeralKey::from_secret(scalar(SK2_PCD), &g);
    assert_eq!(k2.public_bytes().to_vec(), hex(PK2_PCD));

    let kd = x_coordinate(&k2.agree(&decode_point(&hex(PK2_PICC)).unwrap()).unwrap());
    assert_eq!(kd.to_vec(), hex(KD));
}

#[test]
fn session_keys_and_tokens() {
    let keys = SessionKeys::derive(&hex(KD));
    assert_eq!(keys.k_enc, key16(K_ENC));
    assert_eq!(keys.k_mac, key16(K_MAC));
    // KDF counters are the only difference between K_enc and K_mac.
    assert_eq!(*kdf(&hex(KD), 1), key16(K_ENC));

    let oid = Oid(OID.to_vec());
    assert_eq!(
        auth_token(&keys.k_mac, &oid, &hex(PK2_PICC)).to_vec(),
        hex(T_PCD)
    );
    assert_eq!(
        auth_token(&keys.k_mac, &oid, &hex(PK2_PCD)).to_vec(),
        hex(T_PICC)
    );
}

struct FixedKeys(VecDeque<NonZeroScalar>);

impl KeySource for FixedKeys {
    fn next_key(&mut self, g: &ProjectivePoint) -> Result<EphemeralKey, PaceError> {
        let sk = self.0.pop_front().ok_or(PaceError::Rng)?;
        Ok(EphemeralKey::from_secret(sk, g))
    }
}

fn concat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// Card side of the worked example as an APDU script.
fn script(t_picc: &[u8]) -> MockTransport {
    let ok = [0x90, 0x00];
    MockTransport::new([0x3B, 0x80, 0x80, 0x01, 0x01])
        .expect(
            concat(&[
                &[0x00, 0x22, 0xC1, 0xA4, 0x0F, 0x80, 0x0A],
                &OID,
                &[0x83, 0x01, 0x01],
            ]),
            ok,
        )
        .expect(
            [0x10, 0x86, 0x00, 0x00, 0x02, 0x7C, 0x00, 0x00],
            concat(&[&[0x7C, 0x12, 0x80, 0x10], &hex(Z), &ok]),
        )
        .expect(
            concat(&[
                &[0x10, 0x86, 0x00, 0x00, 0x45, 0x7C, 0x43, 0x81, 0x41],
                &hex(PK1_PCD),
                &[0x00],
            ]),
            concat(&[&[0x7C, 0x43, 0x82, 0x41], &hex(PK1_PICC), &ok]),
        )
        .expect(
            concat(&[
                &[0x10, 0x86, 0x00, 0x00, 0x45, 0x7C, 0x43, 0x83, 0x41],
                &hex(PK2_PCD),
                &[0x00],
            ]),
            concat(&[&[0x7C, 0x43, 0x84, 0x41], &hex(PK2_PICC), &ok]),
        )
        .expect(
            concat(&[
                &[0x00, 0x86, 0x00, 0x00, 0x0C, 0x7C, 0x0A, 0x85, 0x08],
                &hex(T_PCD),
                &[0x00],
            ]),
            concat(&[&[0x7C, 0x0A, 0x86, 0x08], t_picc, &ok]),
        )
}

fn pace_info() -> PaceInfo {
    PaceInfo {
        oid: Oid(OID.to_vec()),
        version: 2,
        parameter_id: Some(13),
    }
}

fn fixed_keys() -> FixedKeys {
    FixedKeys(VecDeque::from([scalar(SK1_PCD), scalar(SK2_PCD)]))
}

#[test]
fn full_flow_verifies_t_picc() {
    let mut card = script(&hex(T_PICC));
    let session = establish_with(&mut card, &pace_info(), &mrz(), &mut fixed_keys()).unwrap();
    assert!(card.is_done());
    assert_eq!(session.keys.k_enc, key16(K_ENC));
    assert_eq!(session.keys.k_mac, key16(K_MAC));
    assert_eq!(session.id_picc.to_vec(), hex(PK2_PICC)[1..33]);
}

#[test]
fn full_flow_rejects_wrong_t_picc() {
    let mut bad = hex(T_PICC);
    bad[7] ^= 0x01;
    let mut card = script(&bad);
    let err = establish_with(&mut card, &pace_info(), &mrz(), &mut fixed_keys()).unwrap_err();
    assert!(matches!(err, PaceError::TokenMismatch), "{err:?}");
}

#[test]
fn wrong_password_reported() {
    // Card rejects T_PCD in step 4 (as it would for a wrong CAN).
    let ok = [0x90, 0x00];
    let mut card = MockTransport::new([0x3B])
        .expect(
            concat(&[
                &[0x00, 0x22, 0xC1, 0xA4, 0x0F, 0x80, 0x0A],
                &OID,
                &[0x83, 0x01, 0x01],
            ]),
            ok,
        )
        .expect(
            [0x10, 0x86, 0x00, 0x00, 0x02, 0x7C, 0x00, 0x00],
            concat(&[&[0x7C, 0x12, 0x80, 0x10], &hex(Z), &ok]),
        )
        .expect(
            concat(&[
                &[0x10, 0x86, 0x00, 0x00, 0x45, 0x7C, 0x43, 0x81, 0x41],
                &hex(PK1_PCD),
                &[0x00],
            ]),
            concat(&[&[0x7C, 0x43, 0x82, 0x41], &hex(PK1_PICC), &ok]),
        )
        .expect(
            concat(&[
                &[0x10, 0x86, 0x00, 0x00, 0x45, 0x7C, 0x43, 0x83, 0x41],
                &hex(PK2_PCD),
                &[0x00],
            ]),
            concat(&[&[0x7C, 0x43, 0x84, 0x41], &hex(PK2_PICC), &ok]),
        )
        .expect(
            concat(&[
                &[0x00, 0x86, 0x00, 0x00, 0x0C, 0x7C, 0x0A, 0x85, 0x08],
                &hex(T_PCD),
                &[0x00],
            ]),
            [0x63, 0x00],
        );
    let err = establish_with(&mut card, &pace_info(), &mrz(), &mut fixed_keys()).unwrap_err();
    assert!(
        matches!(err, PaceError::WrongPassword { retries: None }),
        "{err:?}"
    );
}

#[test]
fn can_validation() {
    assert!(Password::can("123456").is_ok());
    assert!(Password::can("12345").is_err());
    assert!(Password::can("12a456").is_err());
    assert_eq!(
        format!("{:?}", Password::can("123456").unwrap()),
        "Password::Can(***)"
    );
}
