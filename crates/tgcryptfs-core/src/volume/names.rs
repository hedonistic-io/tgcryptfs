use rand::seq::SliceRandom;

/// Word pools for generating random "noun verb adjective" group names.
const NOUNS: &[&str] = &[
    "falcon", "orchid", "beacon", "prism", "quartz", "cobalt", "ember", "glacier", "harbor",
    "summit", "cedar", "marble", "horizon", "lantern", "cascade", "anchor", "crystal", "meadow",
    "phantom", "zenith", "thunder", "silver", "coral", "shadow", "velvet", "aurora", "granite",
    "whisper", "nebula", "ivory",
];

const VERBS: &[&str] = &[
    "drifts", "gleams", "echoes", "soars", "melts", "sparks", "blooms", "ripples", "glides",
    "pulses", "swirls", "hums", "shimmers", "dances", "whispers", "crackles", "flows", "blazes",
    "lingers", "wanders", "spirals", "flickers", "surges", "fades", "dashes", "weaves", "floats",
    "chimes", "rushes", "trembles",
];

const ADJECTIVES: &[&str] = &[
    "quietly", "softly", "swiftly", "gently", "boldly", "brightly", "deeply", "calmly", "fiercely",
    "warmly", "slowly", "sharply", "lightly", "smoothly", "wildly", "darkly", "clearly", "freely",
    "purely", "vastly", "keenly", "thinly", "firmly", "loosely", "faintly", "richly", "coolly",
    "dryly", "thickly", "broadly",
];

/// Generate a random "noun verb adjective" group name.
///
/// Example: "falcon drifts quietly"
pub fn generate_group_name() -> String {
    let mut rng = rand::thread_rng();
    let noun = NOUNS.choose(&mut rng).unwrap();
    let verb = VERBS.choose(&mut rng).unwrap();
    let adj = ADJECTIVES.choose(&mut rng).unwrap();
    format!("{noun} {verb} {adj}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_three_word_name() {
        let name = generate_group_name();
        let parts: Vec<&str> = name.split_whitespace().collect();
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn names_are_random() {
        let n1 = generate_group_name();
        let _n2 = generate_group_name();
        // With 30^3 = 27000 combinations, collision is very unlikely
        // but not impossible. Run multiple times to reduce flakiness.
        let mut all_same = true;
        for _ in 0..10 {
            if generate_group_name() != n1 {
                all_same = false;
                break;
            }
        }
        assert!(!all_same, "generated names should vary");
    }

    #[test]
    fn word_pools_non_empty() {
        assert!(!NOUNS.is_empty());
        assert!(!VERBS.is_empty());
        assert!(!ADJECTIVES.is_empty());
    }
}
