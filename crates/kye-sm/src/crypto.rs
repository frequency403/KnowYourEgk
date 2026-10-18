//! AES-128 helpers for secure messaging.

use aes::Aes128;
use aes::cipher::{Array, BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use cmac::{Cmac, Mac};

/// ISO/IEC 9797-1 padding method 2 (`80 00 ..`) to a multiple of 16 bytes.
pub fn pad(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 16);
    out.extend_from_slice(data);
    out.push(0x80);
    while !out.len().is_multiple_of(16) {
        out.push(0x00);
    }
    out
}

/// Removes ISO/IEC 9797-1 method 2 padding.
pub fn unpad(data: &[u8]) -> Option<&[u8]> {
    let end = data.iter().rposition(|b| *b != 0x00)?;
    (data[end] == 0x80 && data.len() - end <= 16).then(|| &data[..end])
}

/// `IV = AES-ENC(K_enc, SSC)`.
pub fn iv_from_ssc(k_enc: &[u8; 16], ssc: &[u8; 16]) -> [u8; 16] {
    let cipher = Aes128::new(&Array::from(*k_enc));
    let mut block = Array::from(*ssc);
    cipher.encrypt_block(&mut block);
    block.into()
}

/// AES-128-CBC encryption of block-aligned data.
pub fn cbc_encrypt(key: &[u8; 16], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    debug_assert!(data.len().is_multiple_of(16));
    let cipher = Aes128::new(&Array::from(*key));
    let mut prev = *iv;
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.as_chunks::<16>().0 {
        let mut block = Array::<u8, _>::default();
        for (b, (d, p)) in block.iter_mut().zip(chunk.iter().zip(prev.iter())) {
            *b = d ^ p;
        }
        cipher.encrypt_block(&mut block);
        prev = block.into();
        out.extend_from_slice(&prev);
    }
    out
}

/// AES-128-CBC decryption of block-aligned data.
pub fn cbc_decrypt(key: &[u8; 16], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let cipher = Aes128::new(&Array::from(*key));
    let mut prev = *iv;
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.as_chunks::<16>().0 {
        let mut block = Array::<u8, _>::default();
        block.copy_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        out.extend(block.iter().zip(prev.iter()).map(|(b, p)| b ^ p));
        prev.copy_from_slice(chunk);
    }
    out
}

/// AES-CMAC truncated to 8 bytes. The caller pads the input beforehand.
pub fn cmac8(key: &[u8; 16], data: &[u8]) -> [u8; 8] {
    let mut mac = <Cmac<Aes128> as KeyInit>::new(&Array::from(*key));
    mac.update(data);
    let full = mac.finalize().into_bytes();
    let mut out = [0u8; 8];
    out.copy_from_slice(&full[..8]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding() {
        assert_eq!(pad(&[]), [vec![0x80], vec![0; 15]].concat());
        assert_eq!(pad(&[1; 16]).len(), 32);
        assert_eq!(unpad(&pad(&[1, 2, 3])), Some(&[1u8, 2, 3][..]));
        assert_eq!(unpad(&[0u8; 16]), None);
        assert_eq!(unpad(&[1u8; 16]), None);
    }

    #[test]
    fn cbc_roundtrip() {
        let key = [7u8; 16];
        let iv = [9u8; 16];
        let data = pad(b"hello secure messaging");
        let enc = cbc_encrypt(&key, &iv, &data);
        assert_ne!(enc, data);
        assert_eq!(cbc_decrypt(&key, &iv, &enc), data);
    }

    #[test]
    fn aes_cmac_rfc4493() {
        // RFC 4493 example 2 (16-byte message; complete block, no CMAC padding).
        let key: [u8; 16] = [
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let msg = [
            0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
            0x17, 0x2a,
        ];
        assert_eq!(
            cmac8(&key, &msg),
            [0x07, 0x0a, 0x16, 0xb4, 0x6b, 0x4d, 0x41, 0x44]
        );
    }
}
