use std::collections::HashMap;

use super::wordlists::{BITS_PER_WORD, WORDS_PER_REF};
use crate::error::{CoreError, Result};

/// Decode a sequence of English words back to a 256-bit value.
pub fn decode_ref(
    words: &[String],
    _wordlists: &[Vec<String>; 4],
    reverse_lookups: &[HashMap<String, u16>; 4],
) -> Result<[u8; 32]> {
    if words.len() != WORDS_PER_REF {
        return Err(CoreError::SentenceEncoding(format!(
            "expected {WORDS_PER_REF} words, got {}",
            words.len()
        )));
    }

    let mut padded = [0u8; 33];

    for (i, word) in words.iter().enumerate() {
        let list_idx = i % 4;
        let index = reverse_lookups[list_idx].get(word).ok_or_else(|| {
            CoreError::SentenceEncoding(format!(
                "word '{}' not found in wordlist {}",
                word, list_idx
            ))
        })?;

        let bit_offset = i * BITS_PER_WORD;
        set_bits(&mut padded, bit_offset, BITS_PER_WORD, *index);
    }

    let mut result = [0u8; 32];
    result.copy_from_slice(&padded[..32]);
    Ok(result)
}

/// Decode a space-separated sentence string back to a 256-bit value.
pub fn decode_ref_string(
    sentence: &str,
    wordlists: &[Vec<String>; 4],
    reverse_lookups: &[HashMap<String, u16>; 4],
) -> Result<[u8; 32]> {
    let words: Vec<String> = sentence
        .split_whitespace()
        .map(std::string::ToString::to_string)
        .collect();
    decode_ref(&words, wordlists, reverse_lookups)
}

/// Set `count` bits starting at `bit_offset` in a byte array.
fn set_bits(data: &mut [u8], bit_offset: usize, count: usize, value: u16) {
    for i in 0..count {
        let byte_idx = (bit_offset + i) / 8;
        let bit_idx = 7 - ((bit_offset + i) % 8);
        if byte_idx < data.len() {
            if (value >> (count - 1 - i)) & 1 == 1 {
                data[byte_idx] |= 1 << bit_idx;
            } else {
                data[byte_idx] &= !(1 << bit_idx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentence::encode::encode_ref;
    use crate::sentence::wordlists::{build_reverse_lookup, placeholder_wordlist};

    fn test_wordlists() -> [Vec<String>; 4] {
        [
            placeholder_wordlist(0),
            placeholder_wordlist(1),
            placeholder_wordlist(2),
            placeholder_wordlist(3),
        ]
    }

    fn test_reverse_lookups(wl: &[Vec<String>; 4]) -> [HashMap<String, u16>; 4] {
        [
            build_reverse_lookup(&wl[0]),
            build_reverse_lookup(&wl[1]),
            build_reverse_lookup(&wl[2]),
            build_reverse_lookup(&wl[3]),
        ]
    }

    #[test]
    fn encode_decode_roundtrip_zeros() {
        let data = [0u8; 32];
        let wl = test_wordlists();
        let rl = test_reverse_lookups(&wl);
        let words = encode_ref(&data, &wl).unwrap();
        let decoded = decode_ref(&words, &wl, &rl).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn encode_decode_roundtrip_all_ones() {
        let data = [0xFF; 32];
        let wl = test_wordlists();
        let rl = test_reverse_lookups(&wl);
        let words = encode_ref(&data, &wl).unwrap();
        let decoded = decode_ref(&words, &wl, &rl).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn encode_decode_roundtrip_random() {
        let data: [u8; 32] = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54,
            0x32, 0x10, 0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x11, 0x22, 0x33,
            0x44, 0x55, 0x66, 0x77,
        ];
        let wl = test_wordlists();
        let rl = test_reverse_lookups(&wl);
        let words = encode_ref(&data, &wl).unwrap();
        let decoded = decode_ref(&words, &wl, &rl).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn wrong_word_count_errors() {
        let wl = test_wordlists();
        let rl = test_reverse_lookups(&wl);
        let words = vec!["w0_0000".to_string(); 5];
        assert!(decode_ref(&words, &wl, &rl).is_err());
    }

    #[test]
    fn unknown_word_errors() {
        let wl = test_wordlists();
        let rl = test_reverse_lookups(&wl);
        let mut words: Vec<String> = vec!["w0_0000".to_string(); WORDS_PER_REF];
        words[0] = "nonexistent_word".to_string();
        assert!(decode_ref(&words, &wl, &rl).is_err());
    }

    #[test]
    fn string_roundtrip() {
        let data = [0x42; 32];
        let wl = test_wordlists();
        let rl = test_reverse_lookups(&wl);
        let sentence = crate::sentence::encode::encode_ref_string(&data, &wl).unwrap();
        let decoded = decode_ref_string(&sentence, &wl, &rl).unwrap();
        assert_eq!(decoded, data);
    }
}
