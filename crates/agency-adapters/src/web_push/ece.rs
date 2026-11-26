//! Message encryption for Web Push (RFC 8291) in the `aes128gcm` content coding of RFC 8188: the
//! application server's ECDH key and the browser's public key and authentication secret make the key
//! and the nonce of one AES-128-GCM record. The push service only ever sees the ciphertext.
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Nonce};
use hkdf::Hkdf;
use p256::ecdh::diffie_hellman;
use p256::{PublicKey, SecretKey};
use sha2::Sha256;

/// The record size written in the header: one record of at most this many octets.
const RECORD_SIZE: u32 = 4096;
/// The octets of a record besides the message: the padding delimiter and the GCM tag.
const RECORD_OVERHEAD: usize = 1 + 16;
/// The header: salt (16), record size (4), key id length (1), and the 65-octet public key as key id.
const HEADER_LEN: usize = 16 + 4 + 1 + 65;
/// The longest message one record holds (RFC 8291, section 4).
pub const MAX_MESSAGE: usize = RECORD_SIZE as usize - HEADER_LEN - RECORD_OVERHEAD;

/// Why a message could not be encrypted. A subscription whose keys are refused will never take a message.
#[derive(Debug, PartialEq, Eq)]
pub enum EceError {
    /// The browser's public key is not a point of P-256, or its authentication secret is not 16 octets.
    BadSubscriptionKeys,
    TooLong,
}

/// The encrypted body of one push message: the header followed by the record.
///
/// `as_secret` is an ECDH key made for this message alone and `salt` is 16 random octets; both are
/// arguments so that the RFC's own example can be checked.
pub fn encrypt(
    message: &[u8],
    ua_public: &[u8],
    auth_secret: &[u8],
    as_secret: &SecretKey,
    salt: [u8; 16],
) -> Result<Vec<u8>, EceError> {
    if message.len() > MAX_MESSAGE {
        return Err(EceError::TooLong);
    }
    if auth_secret.len() != 16 {
        return Err(EceError::BadSubscriptionKeys);
    }
    let ua_key =
        PublicKey::from_sec1_bytes(ua_public).map_err(|_| EceError::BadSubscriptionKeys)?;
    let as_public = as_secret.public_key().to_sec1_bytes();
    let shared = diffie_hellman(as_secret.to_nonzero_scalar(), ua_key.as_affine());

    let mut key_info = b"WebPush: info\0".to_vec();
    key_info.extend_from_slice(ua_public);
    key_info.extend_from_slice(&as_public);
    let mut ikm = [0u8; 32];
    Hkdf::<Sha256>::new(Some(auth_secret), shared.raw_secret_bytes())
        .expand(&key_info, &mut ikm)
        .map_err(|_| EceError::BadSubscriptionKeys)?;

    let record_keys = Hkdf::<Sha256>::new(Some(&salt), &ikm);
    let mut cek = [0u8; 16];
    let mut nonce = [0u8; 12];
    record_keys
        .expand(b"Content-Encoding: aes128gcm\0", &mut cek)
        .and_then(|()| record_keys.expand(b"Content-Encoding: nonce\0", &mut nonce))
        .map_err(|_| EceError::BadSubscriptionKeys)?;

    // The last (here the only) record ends in the delimiter 0x02.
    let mut record = message.to_vec();
    record.push(0x02);
    let sealed = Aes128Gcm::new((&cek).into())
        .encrypt(Nonce::from_slice(&nonce), record.as_slice())
        .map_err(|_| EceError::BadSubscriptionKeys)?;

    let mut body = Vec::with_capacity(HEADER_LEN + sealed.len());
    body.extend_from_slice(&salt);
    body.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    body.push(u8::try_from(as_public.len()).map_err(|_| EceError::TooLong)?);
    body.extend_from_slice(&as_public);
    body.extend_from_slice(&sealed);
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
    use base64::Engine;

    fn decode(text: &str) -> Vec<u8> {
        B64.decode(text.split_whitespace().collect::<String>())
            .unwrap()
    }

    /// RFC 8291, section 5 and appendix A: the same keys, secret, and salt in, the same bytes out.
    #[test]
    fn it_gives_the_bytes_of_the_rfcs_example() {
        let as_secret =
            SecretKey::from_slice(&decode("yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw")).unwrap();
        let ua_public = decode(
            "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4",
        );
        let salt: [u8; 16] = decode("DGv6ra1nlYgDCS1FRnbzlw").try_into().unwrap();
        let body = encrypt(
            b"When I grow up, I want to be a watermelon",
            &ua_public,
            &decode("BTBZMqHH6r4Tts7J_aSIgg"),
            &as_secret,
            salt,
        )
        .unwrap();
        // The RFC gives the header and the record as two base64url texts; joined as octets they are the body.
        let mut expected = decode(
            "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8",
        );
        expected.extend(decode(
            "8pfeW0KbunFT06SuDKoJH9Ql87S1QUrdirN6GcG7sFz1y1sqLgVi1VhjVkHsUoEsbI_0LpXMuGvnzQ",
        ));
        assert_eq!(body, expected);
    }

    #[test]
    fn it_refuses_keys_that_cannot_be_a_subscription_and_a_message_that_does_not_fit() {
        let as_secret = SecretKey::from_slice(&[7u8; 32]).unwrap();
        let ua_public = as_secret.public_key().to_sec1_bytes();
        let good = |message: &[u8], key: &[u8], auth: &[u8]| {
            encrypt(message, key, auth, &as_secret, [0; 16])
        };
        assert!(good(b"x", &ua_public, &[0; 16]).is_ok());
        assert_eq!(
            good(b"x", &ua_public, &[0; 15]),
            Err(EceError::BadSubscriptionKeys)
        );
        assert_eq!(
            good(b"x", &[4; 65], &[0; 16]),
            Err(EceError::BadSubscriptionKeys)
        );
        assert!(good(&vec![b'a'; MAX_MESSAGE], &ua_public, &[0; 16]).is_ok());
        assert_eq!(
            good(&vec![b'a'; MAX_MESSAGE + 1], &ua_public, &[0; 16]),
            Err(EceError::TooLong)
        );
        assert_eq!(MAX_MESSAGE, 3993, "the RFC's limit");
    }
}
