//! Cryptographic primitives for id-PACE-ECDH-GM-AES-CBC-CMAC-128 on brainpoolP256r1.

use aes::Aes128;
use aes::cipher::{Array, BlockCipherDecrypt, KeyInit};
use bp256::elliptic_curve::point::AffineCoordinates;
use bp256::elliptic_curve::sec1::{FromSec1Point, ToSec1Point};
use bp256::elliptic_curve::{Generate, PrimeField};
use bp256::r1::{AffinePoint, NonZeroScalar, ProjectivePoint};
use bp256::{BrainpoolP256r1, Scalar};
use cmac::{Cmac, Mac};
use sha1::{Digest, Sha1};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::PaceError;
use crate::oid::Oid;

/// AES-128 key.
pub type Key128 = Zeroizing<[u8; 16]>;

/// KDF counter for K_enc (gemSpec_COS N001.530, ICAO 9303-11 9.7.1).
const KDF_ENC: u32 = 1;
/// KDF counter for K_mac.
const KDF_MAC: u32 = 2;
/// KDF counter for K_π.
const KDF_PI: u32 = 3;

/// Length of an uncompressed brainpoolP256r1 point `04 || x || y`.
pub const POINT_LEN: usize = 65;

/// `KDF(secret, counter) = SHA-1(secret || be32(counter))[0..16]` for AES-128.
pub fn kdf(secret: &[u8], counter: u32) -> Key128 {
    let mut h = Sha1::new();
    h.update(secret);
    h.update(counter.to_be_bytes());
    let mut digest = h.finalize();
    let mut key = Zeroizing::new([0u8; 16]);
    key.copy_from_slice(&digest[..16]);
    digest.as_mut_slice().zeroize();
    key
}

/// PACE password.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub enum Password {
    /// Card access number as printed on the card (ASCII digits).
    Can(String),
    /// MRZ information `docNo||check || dateOfBirth||check || dateOfExpiry||check`.
    /// Only used for the ICAO test vectors; the eGK uses the CAN.
    Mrz(String),
}

impl Password {
    /// Creates a CAN password, accepting exactly six ASCII digits.
    pub fn can(can: &str) -> Result<Self, PaceError> {
        let can = can.trim();
        if can.len() == 6 && can.bytes().all(|b| b.is_ascii_digit()) {
            Ok(Self::Can(can.to_owned()))
        } else {
            Err(PaceError::InvalidPassword)
        }
    }

    /// `K_π = KDF(π, 3)` where π is the CAN itself or SHA-1 of the MRZ information.
    pub fn k_pi(&self) -> Key128 {
        match self {
            Self::Can(can) => kdf(can.as_bytes(), KDF_PI),
            Self::Mrz(mrz) => {
                let mut pi = Sha1::digest(mrz.as_bytes());
                let k = kdf(&pi, KDF_PI);
                pi.as_mut_slice().zeroize();
                k
            }
        }
    }

    /// MSE:Set AT key reference of this password.
    pub fn key_ref(&self) -> crate::apdu::PasswordRef {
        match self {
            Self::Can(_) => crate::apdu::PasswordRef::Can,
            Self::Mrz(_) => crate::apdu::PasswordRef::Mrz,
        }
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Can(_) => f.write_str("Password::Can(***)"),
            Self::Mrz(_) => f.write_str("Password::Mrz(***)"),
        }
    }
}

/// Decrypts the encrypted nonce: `s = AES-CBC-DEC(K_π, z)` with a zero IV.
pub fn decrypt_nonce(k_pi: &[u8; 16], z: &[u8]) -> Result<Zeroizing<Vec<u8>>, PaceError> {
    if z.is_empty() || !z.len().is_multiple_of(16) {
        return Err(PaceError::GaResponse("encrypted nonce has invalid length"));
    }
    let cipher = Aes128::new(&Array::from(*k_pi));
    let mut out = Zeroizing::new(Vec::with_capacity(z.len()));
    let mut prev = [0u8; 16];
    for chunk in z.chunks_exact(16) {
        let mut block = Array::<u8, _>::default();
        block.copy_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        out.extend(block.iter().zip(prev.iter()).map(|(b, p)| b ^ p));
        prev.copy_from_slice(chunk);
    }
    Ok(out)
}

/// Converts the nonce s into a scalar (big-endian integer, s < 2^128 < n).
pub fn nonce_scalar(s: &[u8]) -> Result<Scalar, PaceError> {
    if s.len() > 32 {
        return Err(PaceError::GaResponse("nonce longer than the group order"));
    }
    let mut repr = <Scalar as PrimeField>::Repr::default();
    repr[32 - s.len()..].copy_from_slice(s);
    let scalar = Scalar::from_repr(repr).into_option();
    repr.as_mut_slice().zeroize();
    scalar.ok_or(PaceError::GaResponse("nonce is not a valid scalar"))
}

/// An ephemeral key pair on some generator. The secret is zeroized on drop.
pub struct EphemeralKey {
    secret: NonZeroScalar,
    public: AffinePoint,
}

impl EphemeralKey {
    /// Generates a random key pair on `generator` using the OS RNG.
    pub fn generate(generator: &ProjectivePoint) -> Result<Self, PaceError> {
        let secret = NonZeroScalar::try_generate().map_err(|_| PaceError::Rng)?;
        Ok(Self::from_secret(secret, generator))
    }

    /// Builds a key pair from a known secret (test vectors).
    pub fn from_secret(secret: NonZeroScalar, generator: &ProjectivePoint) -> Self {
        let public = (*generator * *secret).to_affine();
        Self { secret, public }
    }

    pub fn public(&self) -> &AffinePoint {
        &self.public
    }

    pub fn public_bytes(&self) -> [u8; POINT_LEN] {
        encode_point(&self.public)
    }

    /// Scalar multiplication of the peer's public key with our secret.
    pub fn agree(&self, peer: &AffinePoint) -> Result<AffinePoint, PaceError> {
        let shared = (ProjectivePoint::from(*peer) * *self.secret).to_affine();
        if bool::from(shared.is_identity()) {
            return Err(PaceError::InvalidPoint);
        }
        Ok(shared)
    }
}

impl Drop for EphemeralKey {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

/// Encodes an affine point uncompressed (`04 || x || y`).
pub fn encode_point(p: &AffinePoint) -> [u8; POINT_LEN] {
    let mut out = [0u8; POINT_LEN];
    out.copy_from_slice(p.to_sec1_point(false).as_bytes());
    out
}

/// Decodes and validates an uncompressed point; rejects identity and off-curve points.
pub fn decode_point(bytes: &[u8]) -> Result<AffinePoint, PaceError> {
    if bytes.len() != POINT_LEN || bytes[0] != 0x04 {
        return Err(PaceError::InvalidPoint);
    }
    let point = AffinePoint::from_sec1_bytes(bytes).map_err(|_| PaceError::InvalidPoint)?;
    if bool::from(point.is_identity()) {
        return Err(PaceError::InvalidPoint);
    }
    Ok(point)
}

/// Generic mapping: `G~ = s·G + H` with the shared point H from the first ECDH.
pub fn generic_mapping(s: &Scalar, h: &AffinePoint) -> Result<ProjectivePoint, PaceError> {
    let g = ProjectivePoint::GENERATOR * s + ProjectivePoint::from(*h);
    if bool::from(g.to_affine().is_identity()) {
        return Err(PaceError::InvalidPoint);
    }
    Ok(g)
}

/// x-coordinate of a point as 32 big-endian bytes.
pub fn x_coordinate(p: &AffinePoint) -> Zeroizing<[u8; 32]> {
    let mut x = Zeroizing::new([0u8; 32]);
    x.copy_from_slice(&p.x());
    x
}

/// Session keys derived from the PACE shared secret.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SessionKeys {
    pub k_enc: [u8; 16],
    pub k_mac: [u8; 16],
}

impl SessionKeys {
    /// `K_enc = KDF(KD, 1)`, `K_mac = KDF(KD, 2)`.
    pub fn derive(kd: &[u8]) -> Self {
        Self {
            k_enc: *kdf(kd, KDF_ENC),
            k_mac: *kdf(kd, KDF_MAC),
        }
    }
}

impl std::fmt::Debug for SessionKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionKeys(***)")
    }
}

/// Authentication token `CMAC(K_mac, 7F49 L [06 L OID || 86 41 PK])[0..8]`.
pub fn auth_token(k_mac: &[u8; 16], protocol: &Oid, peer_public: &[u8]) -> [u8; 8] {
    let mut inner = kye_apdu::tlv::encode(0x06, protocol.as_bytes());
    kye_apdu::tlv::write(&mut inner, 0x86, peer_public);
    let template = kye_apdu::tlv::encode(0x7F49, &inner);

    let mut mac = <Cmac<Aes128> as KeyInit>::new(&Array::from(*k_mac));
    mac.update(&template);
    let full = mac.finalize().into_bytes();
    let mut token = [0u8; 8];
    token.copy_from_slice(&full[..8]);
    token
}

/// Constant-time comparison of two authentication tokens.
pub fn tokens_equal(a: &[u8], b: &[u8]) -> bool {
    use bp256::elliptic_curve::subtle::ConstantTimeEq;
    a.len() == b.len() && bool::from(a.ct_eq(b))
}

/// Curve marker, re-exported so callers do not need a direct bp256 dependency.
pub type Curve = BrainpoolP256r1;
