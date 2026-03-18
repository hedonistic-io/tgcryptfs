/// Each wordlist contains 4096 (2^12) words.
/// 4 wordlists × 12 bits = 48 bits per sentence word group.
/// A full 256-bit reference requires ceil(256/12) ≈ 22 words.
///
/// For the initial implementation, we use placeholder lists.
/// Production lists will be generated from curated English word corpora.
pub const WORDLIST_SIZE: usize = 4096;

/// Number of bits encoded per word.
pub const BITS_PER_WORD: usize = 12;

/// Number of words needed to encode 256 bits.
pub const WORDS_PER_REF: usize = 22; // ceil(256 / 12) = 22 (264 bits, 8 padding)

/// Total bits encoded (includes 8 padding bits).
pub const TOTAL_BITS: usize = WORDS_PER_REF * BITS_PER_WORD; // 264

/// Generate a deterministic placeholder wordlist for testing.
/// In production, these would be curated English word lists.
pub fn placeholder_wordlist(list_index: usize) -> Vec<String> {
    (0..WORDLIST_SIZE)
        .map(|i| format!("w{list_index}_{i:04}"))
        .collect()
}

/// Lookup table: word → index within a wordlist.
pub fn build_reverse_lookup(wordlist: &[String]) -> std::collections::HashMap<String, u16> {
    wordlist
        .iter()
        .enumerate()
        .map(|(i, w)| (w.clone(), i as u16))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_wordlist_has_correct_size() {
        let wl = placeholder_wordlist(0);
        assert_eq!(wl.len(), WORDLIST_SIZE);
    }

    #[test]
    fn placeholder_words_unique() {
        let wl = placeholder_wordlist(0);
        let set: std::collections::HashSet<_> = wl.iter().collect();
        assert_eq!(set.len(), WORDLIST_SIZE);
    }

    #[test]
    fn reverse_lookup_consistent() {
        let wl = placeholder_wordlist(0);
        let rl = build_reverse_lookup(&wl);
        for (i, word) in wl.iter().enumerate() {
            assert_eq!(*rl.get(word).unwrap(), i as u16);
        }
    }
}
