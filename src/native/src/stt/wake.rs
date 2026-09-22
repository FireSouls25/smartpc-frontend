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

/// Split a transcribed onset into wake match + remainder. Returns `None`
/// when the wake word isn't in it, else the text after the match (possibly
/// empty: bare "hey" with the command still to come).
///
/// Single words match whole-word ("they" ≠ "hey", "¡Hey!" = "hey");
/// multi-word phrases match as substrings on the normalized text.
pub fn split_wake_command(transcript: &str, word: &str) -> Option<String> {
    let hay = normalize(transcript);
    let needle = normalize(word);
    if needle.is_empty() {
        return None;
    }
    if needle.contains(' ') {
        return hay
            .find(&needle)
            .map(|i| hay[i + needle.len()..].trim().to_string());
    }
    let mut rest: Vec<&str> = Vec::new();
    let mut found = false;
    for token in hay.split_whitespace() {
        if !found && token == needle {
            found = true;
            continue;
        }
        if found {
            rest.push(token);
        }
    }
    found.then(|| rest.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hey_variants_match() {
        assert!(split_wake_command("Hey, what's the time?", "hey").is_some());
        assert!(split_wake_command("¡HEY!", "hey").is_some());
        assert!(split_wake_command("  hey  ", "hey").is_some());
        assert!(split_wake_command("HEY", "Hey").is_some());
    }

    #[test]
    fn whole_word_only() {
        assert_eq!(split_wake_command("they went home", "hey"), None);
        assert_eq!(split_wake_command("heyday", "hey"), None);
        assert_eq!(split_wake_command("whey protein", "hey"), None);
    }

    #[test]
    fn one_breath_commands_come_back_as_remainder() {
        assert_eq!(
            split_wake_command("hey, what's the time?", "hey"),
            Some("what s the time".to_string())
        );
        assert_eq!(
            split_wake_command("hey computer, open the terminal", "hey computer"),
            Some("open the terminal".to_string())
        );
        // Bare wake word: matched, nothing after it yet.
        assert_eq!(
            split_wake_command("hey", "hey"),
            Some(String::new())
        );
        assert_eq!(
            split_wake_command("hey…", "hey"),
            Some(String::new())
        );
    }

    #[test]
    fn phrases_match_as_substrings() {
        assert!(split_wake_command(
            "hey, computer, open the terminal",
            "hey computer"
        )
        .is_some());
        assert_eq!(
            split_wake_command("hey there", "hey computer"),
            None
        );
    }

    #[test]
    fn empty_never_matches() {
        assert_eq!(split_wake_command("hey you", ""), None);
        assert_eq!(split_wake_command("hey you", "   "), None);
        assert_eq!(split_wake_command("", "hey"), None);
    }

    #[test]
    fn punctuation_and_case_are_ignored() {
        assert!(split_wake_command("¿Qué tal? ¡Oye!", "oye").is_some());
        assert_eq!(
            split_wake_command("OYE, abre chrome", "oye"),
            Some("abre chrome".to_string())
        );
    }
}
