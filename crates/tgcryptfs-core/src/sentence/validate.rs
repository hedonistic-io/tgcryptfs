use super::wordlists::WORDS_PER_REF;
use std::collections::HashMap;

/// Validate that a sentence reference has the correct structure.
pub fn validate_sentence(
    words: &[String],
    reverse_lookups: &[HashMap<String, u16>; 4],
) -> Result<(), String> {
    if words.len() != WORDS_PER_REF {
        return Err(format!(
            "expected {WORDS_PER_REF} words, got {}",
            words.len()
        ));
    }

    for (i, word) in words.iter().enumerate() {
        let list_idx = i % 4;
        if !reverse_lookups[list_idx].contains_key(word) {
            return Err(format!(
                "word '{}' at position {} not found in wordlist {}",
                word, i, list_idx
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentence::wordlists::{build_reverse_lookup, placeholder_wordlist};

    fn test_reverse_lookups() -> [HashMap<String, u16>; 4] {
        [
            build_reverse_lookup(&placeholder_wordlist(0)),
            build_reverse_lookup(&placeholder_wordlist(1)),
            build_reverse_lookup(&placeholder_wordlist(2)),
            build_reverse_lookup(&placeholder_wordlist(3)),
        ]
    }

    #[test]
    fn valid_sentence() {
        let rl = test_reverse_lookups();
        let words: Vec<String> = (0..WORDS_PER_REF)
            .map(|i| format!("w{}_0000", i % 4))
            .collect();
        assert!(validate_sentence(&words, &rl).is_ok());
    }

    #[test]
    fn wrong_count_rejected() {
        let rl = test_reverse_lookups();
        let words = vec!["w0_0000".to_string(); 5];
        assert!(validate_sentence(&words, &rl).is_err());
    }

    #[test]
    fn invalid_word_rejected() {
        let rl = test_reverse_lookups();
        let mut words: Vec<String> = (0..WORDS_PER_REF)
            .map(|i| format!("w{}_0000", i % 4))
            .collect();
        words[3] = "INVALID".to_string();
        assert!(validate_sentence(&words, &rl).is_err());
    }
}
