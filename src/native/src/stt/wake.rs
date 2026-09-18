//! Wake-word matching over whisper transcripts.
//!
//! Deliberately textual, not acoustic: the VAD gates *when* we listen, tiny
//! transcribes *what* was said, and this decides whether the wake word is in
//! it. Costs one tiny transcription per speech onset in wake mode (nothing
//! runs continuously), all on-device. Limitations, stated plainly:
//!
//! - "hey" also fires on "hey!" mid-sentence — by design (any mention wakes).
//!   Whole-word matching avoids "they"/"heyday" false positives.
//! - Accents are significant ("qué" ≠ "que"); the default "hey" is unaffected.
//! - Multi-word phrases match as substrings ("hey computer" in "...hey,
//!   computer, ...").

/// Normalize for matching: lowercase, keep alphanumerics, squash every
/// other run into a single space.
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut gap = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
            gap = false;
        } else if !gap {
            out.push(' ');
            gap = true;
        }
    }
    out.trim_end().to_string()
}

/// True when `word` (single word → whole-word match, phrase → substring) is
/// in the transcript. Empty patterns never match.
pub fn contains_wake_word(transcript: &str, word: &str) -> bool {
    let hay = normalize(transcript);
    let needle = normalize(word);
    if needle.is_empty() {
        return false;
    }
    if needle.contains(' ') {
        hay.contains(&needle)
    } else {
        hay.split_whitespace().any(|t| t == needle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hey_variants_match() {
        assert!(contains_wake_word("Hey, what's the time?", "hey"));
        assert!(contains_wake_word("¡HEY!", "hey"));
        assert!(contains_wake_word("  hey  ", "hey"));
        assert!(contains_wake_word("HEY", "Hey"));
    }

    #[test]
    fn whole_word_only() {
        assert!(!contains_wake_word("they went home", "hey"));
        assert!(!contains_wake_word("heyday", "hey"));
        assert!(!contains_wake_word("whey protein", "hey"));
    }

    #[test]
    fn phrases_match_as_substrings() {
        assert!(contains_wake_word(
            "hey, computer, open the terminal",
            "hey computer"
        ));
        assert!(!contains_wake_word("hey there", "hey computer"));
    }

    #[test]
    fn empty_never_matches() {
        assert!(!contains_wake_word("hey you", ""));
        assert!(!contains_wake_word("hey you", "   "));
        assert!(!contains_wake_word("", "hey"));
    }

    #[test]
    fn punctuation_and_case_are_ignored() {
        assert!(contains_wake_word("¿Qué tal? ¡Oye!", "oye"));
        assert!(contains_wake_word("OYE, abre chrome", "oye"));
    }
}
