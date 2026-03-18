use crate::crypto::keys::SymmetricKey;
use crate::error::{CoreError, Result};

use kem::{Decapsulate, Encapsulate};
use ml_kem::{Encoded, EncodedSizeUser, KemCore, MlKem768};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// ML-KEM-768 encapsulation key (public key), serialized.
#[derive(Debug, Clone)]
pub struct MlKemEncapsulationKey(pub Vec<u8>);

/// ML-KEM-768 decapsulation key (secret key), serialized.
/// Zeroized on drop to prevent key material from lingering in memory.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct MlKemDecapsulationKey(pub Vec<u8>);

impl std::fmt::Debug for MlKemDecapsulationKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("MlKemDecapsulationKey")
            .field(&"[REDACTED]")
            .finish()
    }
}

/// ML-KEM-768 ciphertext (encapsulated shared secret), serialized.
#[derive(Debug, Clone)]
pub struct MlKemCiphertext(pub Vec<u8>);

/// Generate an ML-KEM-768 keypair.
pub fn generate_keypair() -> Result<(MlKemDecapsulationKey, MlKemEncapsulationKey)> {
    let mut rng = rand::rngs::OsRng;
    let (dk, ek) = MlKem768::generate(&mut rng);

    let mut dk_bytes = dk.as_bytes().to_vec();
    let ek_bytes = ek.as_bytes().to_vec();

    let result = (
        MlKemDecapsulationKey(dk_bytes.clone()),
        MlKemEncapsulationKey(ek_bytes),
    );
    // Zeroize the intermediate copy so only the wrapper holds key material
    dk_bytes.zeroize();
    Ok(result)
}

/// Encapsulate: produce a shared secret and ciphertext from a public key.
pub fn encapsulate(ek: &MlKemEncapsulationKey) -> Result<(SymmetricKey, MlKemCiphertext)> {
    type EK = <MlKem768 as KemCore>::EncapsulationKey;

    let ek_encoded: &Encoded<EK> =
        ek.0.as_slice()
            .try_into()
            .map_err(|_| CoreError::InvalidKey("invalid encapsulation key length".into()))?;

    let ek_parsed = EK::from_bytes(ek_encoded);

    let mut rng = rand::rngs::OsRng;
    let (ct, ss) = ek_parsed
        .encapsulate(&mut rng)
        .map_err(|e| CoreError::Encryption(format!("encapsulate: {e:?}")))?;

    let ct_bytes = ct.to_vec();
    let mut ss_bytes = [0u8; 32];
    ss_bytes.copy_from_slice(ss.as_slice());
    let key = SymmetricKey::from_bytes(ss_bytes);
    ss_bytes.zeroize();

    Ok((key, MlKemCiphertext(ct_bytes)))
}

/// Decapsulate: recover the shared secret from a ciphertext and secret key.
pub fn decapsulate(dk: &MlKemDecapsulationKey, ct: &MlKemCiphertext) -> Result<SymmetricKey> {
    type DK = <MlKem768 as KemCore>::DecapsulationKey;
    type CT = ml_kem::Ciphertext<MlKem768>;

    let dk_encoded: &Encoded<DK> =
        dk.0.as_slice()
            .try_into()
            .map_err(|_| CoreError::InvalidKey("invalid decapsulation key length".into()))?;

    let dk_parsed = DK::from_bytes(dk_encoded);

    let ct_parsed: &CT =
        ct.0.as_slice()
            .try_into()
            .map_err(|_| CoreError::InvalidKey("invalid ciphertext length".into()))?;

    let ss = dk_parsed
        .decapsulate(ct_parsed)
        .map_err(|e| CoreError::Decryption(format!("decapsulate: {e:?}")))?;

    let mut ss_bytes = [0u8; 32];
    ss_bytes.copy_from_slice(ss.as_slice());
    let key = SymmetricKey::from_bytes(ss_bytes);
    ss_bytes.zeroize();

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_generation() {
        let (dk, ek) = generate_keypair().unwrap();
        assert!(!dk.0.is_empty());
        assert!(!ek.0.is_empty());
    }

    #[test]
    fn encapsulate_decapsulate_roundtrip() {
        let (dk, ek) = generate_keypair().unwrap();
        let (ss_sender, ct) = encapsulate(&ek).unwrap();
        let ss_receiver = decapsulate(&dk, &ct).unwrap();
        assert_eq!(ss_sender.as_bytes(), ss_receiver.as_bytes());
    }

    #[test]
    fn different_encapsulations_produce_different_secrets() {
        let (_dk, ek) = generate_keypair().unwrap();
        let (ss1, _ct1) = encapsulate(&ek).unwrap();
        let (ss2, _ct2) = encapsulate(&ek).unwrap();
        assert_ne!(ss1.as_bytes(), ss2.as_bytes());
    }

    #[test]
    fn wrong_decapsulation_key_produces_different_secret() {
        let (_dk1, ek1) = generate_keypair().unwrap();
        let (dk2, _ek2) = generate_keypair().unwrap();
        let (ss1, ct1) = encapsulate(&ek1).unwrap();
        let ss_wrong = decapsulate(&dk2, &ct1).unwrap();
        assert_ne!(ss1.as_bytes(), ss_wrong.as_bytes());
    }
}
