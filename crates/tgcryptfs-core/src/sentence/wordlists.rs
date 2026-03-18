/// Each wordlist contains 4096 (2^12) words.
/// 4 wordlists × 12 bits = 48 bits per sentence word group.
/// A full 256-bit reference requires ceil(256/12) ≈ 22 words.
pub const WORDLIST_SIZE: usize = 4096;

/// Number of bits encoded per word.
pub const BITS_PER_WORD: usize = 12;

/// Number of words needed to encode 256 bits.
pub const WORDS_PER_REF: usize = 22; // ceil(256 / 12) = 22 (264 bits, 8 padding)

/// Total bits encoded (includes 8 padding bits).
pub const TOTAL_BITS: usize = WORDS_PER_REF * BITS_PER_WORD; // 264

/// Get a production wordlist by index (0-3).
/// Returns a Vec<String> for compatibility with the encode/decode API.
pub fn production_wordlist(list_index: usize) -> Vec<String> {
    use super::wordlists_data;
    let list: &[&str; 4096] = match list_index {
        0 => wordlists_data::WORDLIST_0,
        1 => wordlists_data::WORDLIST_1,
        2 => wordlists_data::WORDLIST_2,
        3 => wordlists_data::WORDLIST_3,
        _ => panic!("wordlist index must be 0-3, got {list_index}"),
    };
    list.iter().map(|s| (*s).to_string()).collect()
}

/// Generate a deterministic placeholder wordlist for testing.
/// Retained for backward compatibility in tests.
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
    fn production_wordlists_have_correct_size() {
        for i in 0..4 {
            let wl = production_wordlist(i);
            assert_eq!(wl.len(), WORDLIST_SIZE, "wordlist {i} has wrong size");
        }
    }

    #[test]
    fn production_wordlists_unique_entries() {
        for i in 0..4 {
            let wl = production_wordlist(i);
            let set: std::collections::HashSet<_> = wl.iter().collect();
            assert_eq!(set.len(), WORDLIST_SIZE, "wordlist {i} has duplicates");
        }
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
