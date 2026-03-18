use super::wordlists::{self, BITS_PER_WORD, WORDS_PER_REF};
use crate::error::{CoreError, Result};

/// Encode a 256-bit value as a sequence of English words (sentence reference).
///
/// Each word encodes 12 bits (index into a 4096-word list).
/// Words alternate between wordlists in round-robin fashion.
pub fn encode_ref(data: &[u8; 32], wordlists: &[Vec<String>; 4]) -> Result<Vec<String>> {
    if wordlists
        .iter()
        .any(|wl| wl.len() < wordlists::WORDLIST_SIZE)
    {
        return Err(CoreError::SentenceEncoding("wordlist too small".into()));
    }

    // Pad to 264 bits (33 bytes) with a zero byte
    let mut padded = [0u8; 33];
    padded[..32].copy_from_slice(data);

    let mut words = Vec::with_capacity(WORDS_PER_REF);

    for i in 0..WORDS_PER_REF {
        let bit_offset = i * BITS_PER_WORD;
        let index = extract_bits(&padded, bit_offset, BITS_PER_WORD);
        let list_idx = i % 4;
        words.push(wordlists[list_idx][index as usize].clone());
    }

    Ok(words)
}

/// Encode a 256-bit value as a space-separated sentence string.
pub fn encode_ref_string(data: &[u8; 32], wordlists: &[Vec<String>; 4]) -> Result<String> {
    let words = encode_ref(data, wordlists)?;
    Ok(words.join(" "))
}

/// Extract `count` bits starting at `bit_offset` from a byte array.
fn extract_bits(data: &[u8], bit_offset: usize, count: usize) -> u16 {
    let mut value: u16 = 0;
    for i in 0..count {
        let byte_idx = (bit_offset + i) / 8;
        let bit_idx = 7 - ((bit_offset + i) % 8);
        if byte_idx < data.len() && (data[byte_idx] >> bit_idx) & 1 == 1 {
            value |= 1 << (count - 1 - i);
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentence::wordlists::placeholder_wordlist;

    fn test_wordlists() -> [Vec<String>; 4] {
        [
            placeholder_wordlist(0),
            placeholder_wordlist(1),
            placeholder_wordlist(2),
            placeholder_wordlist(3),
        ]
    }

    #[test]
    fn encode_produces_correct_word_count() {
        let data = [0u8; 32];
        let words = encode_ref(&data, &test_wordlists()).unwrap();
        assert_eq!(words.len(), WORDS_PER_REF);
    }

    #[test]
    fn encode_all_zeros() {
        let data = [0u8; 32];
        let words = encode_ref(&data, &test_wordlists()).unwrap();
        // All zero bits → all index 0
        for (i, word) in words.iter().enumerate() {
            let list_idx = i % 4;
            assert_eq!(*word, format!("w{list_idx}_0000"));
        }
    }

    #[test]
    fn encode_deterministic() {
        let data = [0xAB; 32];
        let wl = test_wordlists();
        let w1 = encode_ref(&data, &wl).unwrap();
        let w2 = encode_ref(&data, &wl).unwrap();
        assert_eq!(w1, w2);
    }

    #[test]
    fn encode_different_data_different_words() {
        let wl = test_wordlists();
        let w1 = encode_ref(&[0x00; 32], &wl).unwrap();
        let w2 = encode_ref(&[0xFF; 32], &wl).unwrap();
        assert_ne!(w1, w2);
    }

    #[test]
    fn encode_string_format() {
        let data = [0u8; 32];
        let s = encode_ref_string(&data, &test_wordlists()).unwrap();
        assert!(s.contains(' '));
        assert_eq!(s.split_whitespace().count(), WORDS_PER_REF);
    }

    #[test]
    fn extract_bits_basic() {
        let data = [0b10110000];
        assert_eq!(extract_bits(&data, 0, 4), 0b1011);
        assert_eq!(extract_bits(&data, 4, 4), 0b0000);
    }

    #[test]
    fn extract_bits_cross_byte() {
        let data = [0b11110000, 0b00001111];
        assert_eq!(extract_bits(&data, 4, 8), 0b00000000);
        assert_eq!(extract_bits(&data, 0, 8), 0b11110000);
    }
}
