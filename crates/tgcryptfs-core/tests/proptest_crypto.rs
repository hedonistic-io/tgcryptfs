use std::collections::HashMap;

use proptest::prelude::*;

use tgcryptfs_core::crypto::aead;
use tgcryptfs_core::crypto::kdf;
use tgcryptfs_core::crypto::keys::SymmetricKey;
use tgcryptfs_core::sentence;

/// Generate a random 32-byte key for testing.
fn arb_key() -> impl Strategy<Value = SymmetricKey> {
    prop::array::uniform32(any::<u8>()).prop_map(SymmetricKey::from_bytes)
}

proptest! {
    /// AEAD encrypt/decrypt roundtrip: decrypt(encrypt(pt)) == pt
    #[test]
    fn aead_roundtrip(
        key in arb_key(),
        plaintext in prop::collection::vec(any::<u8>(), 0..4096),
        aad in prop::collection::vec(any::<u8>(), 0..256),
    ) {
        let ct = aead::encrypt(&key, &plaintext, &aad).unwrap();
        let pt2 = aead::decrypt(&key, &ct, &aad).unwrap();
        prop_assert_eq!(&plaintext, &pt2);
    }

    /// Wrong key fails decryption.
    #[test]
    fn aead_wrong_key_fails(
        key1 in arb_key(),
        key2 in arb_key(),
        plaintext in prop::collection::vec(any::<u8>(), 1..512),
        aad in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        // Skip if keys happen to be equal (astronomically unlikely)
        prop_assume!(key1.as_bytes() != key2.as_bytes());
        let ct = aead::encrypt(&key1, &plaintext, &aad).unwrap();
        let result = aead::decrypt(&key2, &ct, &aad);
        prop_assert!(result.is_err());
    }

    /// Wrong AAD fails decryption.
    #[test]
    fn aead_wrong_aad_fails(
        key in arb_key(),
        plaintext in prop::collection::vec(any::<u8>(), 1..512),
        aad1 in prop::collection::vec(any::<u8>(), 1..64),
        aad2 in prop::collection::vec(any::<u8>(), 1..64),
    ) {
        prop_assume!(aad1 != aad2);
        let ct = aead::encrypt(&key, &plaintext, &aad1).unwrap();
        let result = aead::decrypt(&key, &ct, &aad2);
        prop_assert!(result.is_err());
    }

    /// Ciphertext is larger than plaintext (nonce + tag overhead).
    #[test]
    fn aead_ciphertext_overhead(
        key in arb_key(),
        plaintext in prop::collection::vec(any::<u8>(), 0..2048),
    ) {
        let ct = aead::encrypt(&key, &plaintext, b"test").unwrap();
        let overhead = aead::NONCE_SIZE + aead::TAG_SIZE;
        prop_assert_eq!(ct.len(), plaintext.len() + overhead);
    }

    /// KDF: same inputs produce same output (deterministic).
    #[test]
    fn kdf_deterministic(
        parent in arb_key(),
        info in prop::collection::vec(any::<u8>(), 1..128),
    ) {
        let k1 = kdf::hkdf_derive(&parent, &info).unwrap();
        let k2 = kdf::hkdf_derive(&parent, &info).unwrap();
        prop_assert_eq!(k1.as_bytes(), k2.as_bytes());
    }

    /// KDF: different info produces different keys.
    #[test]
    fn kdf_different_info_different_keys(
        parent in arb_key(),
        info1 in prop::collection::vec(any::<u8>(), 1..128),
        info2 in prop::collection::vec(any::<u8>(), 1..128),
    ) {
        prop_assume!(info1 != info2);
        let k1 = kdf::hkdf_derive(&parent, &info1).unwrap();
        let k2 = kdf::hkdf_derive(&parent, &info2).unwrap();
        prop_assert_ne!(k1.as_bytes(), k2.as_bytes());
    }

    /// Epoch keys: different epochs produce different keys.
    #[test]
    fn epoch_keys_differ(
        parent in arb_key(),
        epoch1 in 0u32..1000,
        epoch2 in 0u32..1000,
    ) {
        prop_assume!(epoch1 != epoch2);
        let k1 = kdf::derive_epoch_key(&parent, epoch1).unwrap();
        let k2 = kdf::derive_epoch_key(&parent, epoch2).unwrap();
        prop_assert_ne!(k1.key.as_bytes(), k2.key.as_bytes());
    }

    /// Sentence encoding roundtrip: decode(encode(data)) == data
    #[test]
    fn sentence_roundtrip(
        data in prop::array::uniform32(any::<u8>()),
    ) {
        let wordlists: [Vec<String>; 4] = core::array::from_fn(sentence::wordlists::placeholder_wordlist);
        let reverse: [HashMap<String, u16>; 4] = core::array::from_fn(|i| {
            sentence::wordlists::build_reverse_lookup(&wordlists[i])
        });

        let encoded = sentence::encode::encode_ref_string(&data, &wordlists).unwrap();
        let decoded = sentence::decode::decode_ref_string(&encoded, &wordlists, &reverse).unwrap();
        prop_assert_eq!(&data, &decoded);
    }

    /// Sentence encoding produces exactly 22 words.
    #[test]
    fn sentence_word_count(
        data in prop::array::uniform32(any::<u8>()),
    ) {
        let wordlists: [Vec<String>; 4] = core::array::from_fn(sentence::wordlists::placeholder_wordlist);
        let encoded = sentence::encode::encode_ref_string(&data, &wordlists).unwrap();
        let word_count = encoded.split_whitespace().count();
        prop_assert_eq!(word_count, 22);
    }
}
