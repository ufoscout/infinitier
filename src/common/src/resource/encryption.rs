//! Infinity Engine resource (de)obfuscation.
//!
//! A number of stock game resources — most commonly `IDS` files, but
//! also some `2DA`, `SPL`, etc. — are stored XOR-"encrypted" on disk.
//! An encrypted resource begins with the two-byte marker `0xFF 0xFF`;
//! the remaining bytes are XORed against a fixed 64-byte repeating key.
//! The transform is symmetric, so the same XOR both encrypts and
//! decrypts (decryption additionally strips, and encryption adds, the
//! two-byte marker).
//!
//! Reference:
//! <https://gibberlings3.github.io/iesdp/file_formats/ie_formats/encryption.htm>
//! (key cross-checked against NearInfinity's `Decryptor`).

use std::borrow::Cow;

/// Two-byte marker prefixing an encrypted resource.
pub const ENCRYPTED_HEADER: [u8; 2] = [0xFF, 0xFF];

/// The fixed 64-byte XOR key applied (repeating) to the body of an
/// encrypted resource, after the [`ENCRYPTED_HEADER`].
pub const XOR_KEY: [u8; 64] = [
    0x88, 0xA8, 0x8F, 0xBA, 0x8A, 0xD3, 0xB9, 0xF5, 0xED, 0xB1, 0xCF, 0xEA, 0xAA, 0xE4, 0xB5, 0xFB,
    0xEB, 0x82, 0xF9, 0x90, 0xCA, 0xC9, 0xB5, 0xE7, 0xDC, 0x8E, 0xB7, 0xAC, 0xEE, 0xF7, 0xE0, 0xCA,
    0x8E, 0xEA, 0xCA, 0x80, 0xCE, 0xC5, 0xAD, 0xB7, 0xC4, 0xD0, 0x84, 0x93, 0xD5, 0xF0, 0xEB, 0xC8,
    0xB4, 0x9D, 0xCC, 0xAF, 0xA5, 0x95, 0xBA, 0x99, 0x87, 0xD2, 0x9D, 0xE3, 0x91, 0xBA, 0x90, 0xCA,
];

/// `true` when `data` carries the [`ENCRYPTED_HEADER`] marker.
pub fn is_encrypted(data: &[u8]) -> bool {
    data.starts_with(&ENCRYPTED_HEADER)
}

/// XOR `body` in place against the repeating [`XOR_KEY`]. This is the
/// raw transform applied to the bytes *after* the header.
fn xor_body(body: &mut [u8]) {
    for (i, byte) in body.iter_mut().enumerate() {
        *byte ^= XOR_KEY[i % XOR_KEY.len()];
    }
}

/// Decrypt an encrypted resource: strip the [`ENCRYPTED_HEADER`] and XOR
/// the remaining bytes.
///
/// Takes (and returns) a [`Cow`] so it allocates as little as possible:
/// - plaintext input is returned unchanged (a borrow stays a borrow);
/// - encrypted input that is already **owned** is decrypted in place by
///   reusing its buffer (drain the header, XOR the rest) — no new
///   allocation;
/// - encrypted input that is **borrowed** copies the body once.
///
/// So callers holding a `Vec<u8>` can hand over ownership
/// (`Cow::Owned(buf)`) and get the plaintext back with no extra copy.
pub fn decrypt(data: Cow<'_, [u8]>) -> Cow<'_, [u8]> {
    if !is_encrypted(&data) {
        return data;
    }
    let mut body = match data {
        Cow::Owned(mut buf) => {
            buf.drain(..ENCRYPTED_HEADER.len());
            buf
        }
        Cow::Borrowed(bytes) => bytes[ENCRYPTED_HEADER.len()..].to_vec(),
    };
    xor_body(&mut body);
    Cow::Owned(body)
}

/// Encrypt plaintext: prepend the [`ENCRYPTED_HEADER`] and XOR the body.
/// `encrypt` then `decrypt` round-trips to the original bytes.
pub fn encrypt(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + ENCRYPTED_HEADER.len());
    out.extend_from_slice(&ENCRYPTED_HEADER);
    let start = out.len();
    out.extend_from_slice(data);
    xor_body(&mut out[start..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_detected() {
        assert!(is_encrypted(&[0xFF, 0xFF, 0x00]));
        assert!(!is_encrypted(b"2DA V1.0"));
        assert!(!is_encrypted(&[0xFF]));
        assert!(!is_encrypted(&[]));
    }

    #[test]
    fn round_trips_reusing_owned_buffer() {
        let plain = b"2DA V1.0\r\n0\r\n   COL\r\n0  -5\r\n1  -3\r\n";
        let cipher = encrypt(plain);
        assert!(is_encrypted(&cipher));
        assert_ne!(&cipher[2..], &plain[..]);
        // Hand over ownership: the encrypted buffer is reused in place.
        let decoded = decrypt(Cow::Owned(cipher));
        assert!(matches!(decoded, Cow::Owned(_)));
        assert_eq!(decoded.as_ref(), &plain[..]);
    }

    #[test]
    fn decrypt_passes_through_plaintext_without_copying() {
        let plain = b"2DA V1.0";
        let decoded = decrypt(Cow::Borrowed(plain));
        assert!(
            matches!(decoded, Cow::Borrowed(_)),
            "plaintext must be borrowed, not copied"
        );
        assert_eq!(decoded.as_ref(), &plain[..]);
    }

    #[test]
    fn known_vector_decrypts_to_2da_header() {
        // First bytes of the real (encrypted) STRMOD.2DA from classic BG.
        let cipher = [
            0xFF, 0xFF, 0xA8, 0x9A, 0xCB, 0xFB, 0xAA, 0x85, 0x88, 0xDB, 0xDD, 0xBC,
        ];
        // Borrowed encrypted input copies the body once.
        let plain = decrypt(Cow::Borrowed(&cipher[..]));
        assert!(matches!(plain, Cow::Owned(_)));
        assert_eq!(&plain[..9], b" 2DA V1.0");
    }
}
