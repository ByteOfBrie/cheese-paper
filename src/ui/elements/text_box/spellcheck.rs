use crate::ui::prelude::*;

use cow_utils::CowUtils;

use std::borrow::Cow;
use std::ops::Range;

/// Given a position (character offset), finds the byte range of the word (exclusively bounded by
/// whitespace) that is contained at that position
pub fn get_current_word(text: &str, position: usize) -> Range<usize> {
    let chars: Vec<_> = text.char_indices().collect();

    let mut before_pos_option = None;

    // Starting at the position given, look backwards for whitespace. When we find it, we stop
    // immediately, so we'll end up with the previous character's index.
    //
    // We can't trivially find the first whitespace position and then correct for that because we need
    // the char index. There's almost definitely still a way to do this as a single line, but the
    // current iteration is what I understand. Something like:
    // `chars[..position].iter().rev().position(|(pos, chr)| chr.is_whitespace())` with the right match
    //
    // example: " word " starting at index 2 (`o`)
    // slice is ` w` and then reversed to `w `
    // loop 1: examine `w` (position = 1)
    //         not whitespace, set before_pos_option to Some(1)
    // loop 2: examine ` ` (position = 0)
    //         whitespace, break
    // before_pos = 1
    for (pos, chr) in chars[..position].iter().rev() {
        if chr.is_whitespace() {
            // The last character we found was the correct spot, before_pos_option is already set
            break;
        } else {
            before_pos_option = Some(*pos);
        }
    }

    // if we started on a whitespace character, we'll still have None, so the start of the range is
    // the starting position (in byte offset)
    let before_pos = match before_pos_option {
        Some(pos) => pos,
        None => {
            if position == chars.len() {
                // special case, we're at the end of the file, we can't look at the index
                text.len()
            } else {
                // usual case, we have a "normal" position, find it
                chars[position].0
            }
        }
    };

    // We now go forwards in the string, but consuming characters. Once we find a whitespace character,
    // we use that character's byte offset of the end of our range, since it will be the end of our
    // range. This results in the slice grabbing the full word, but not spaces. If we don't find anything,
    // we use the full length of the text (byte version, not char version)
    let after_pos = chars[position..]
        .iter()
        .find_map(|(pos, chr)| {
            if chr.is_whitespace() {
                Some(*pos)
            } else {
                None
            }
        })
        .unwrap_or(text.len());

    before_pos..after_pos
}

#[test]
fn test_get_current_word() {
    assert_eq!(get_current_word("asdf jkl qwerty", 2), 0..4);
    assert_eq!(get_current_word("asdf jkl qwerty", 4), 0..4);
    assert_eq!(get_current_word("asdf jkl qwerty", 6), 5..8);
    assert_eq!(get_current_word("asdf  qwerty", 5), 5..5);
    assert_eq!(get_current_word("asdf  qwerty", 6), 6..12);
    assert_eq!(get_current_word("ßß ssss", 1), 0..4);
    assert_eq!(get_current_word("ßß ssss", 2), 0..4);
    assert_eq!(get_current_word("ßß ssss", 3), 5..9);
    assert_eq!("Alte Jakobstraße".len(), 17); // String is 17 bytes long
    assert_eq!(get_current_word("Alte Jakobstraße", 5), 5..17);
    assert_eq!(get_current_word("Alte Jakobstrasse", 5), 5..17);

    // end of line, make sure it works
    assert_eq!(get_current_word("Alte Jakobstraße", 16), 5..17);
    assert_eq!(get_current_word("Alte Jakobstrasse", 17), 5..17);
}

pub fn trim_word_for_spellcheck(word: &str) -> (Cow<'_, str>, Range<usize>) {
    // Keep track of how much we trimmed in each step (since that shouldn't be
    // marked as misspelled). This could also be done by a regex, but that seems
    // more complicated
    // possible regex: ^(['".,\-!*_]*)(\w.*\w)?(['".,\-!*_]*)$
    let start_trimmed_word = word.trim_start_matches(|chr: char| chr.is_ascii_punctuation());
    let trimmed_word = start_trimmed_word.trim_end_matches(|chr: char| chr.is_ascii_punctuation());

    // Rare case, allow for mid-word formatting changes (without unnecessary allocation)
    let check_word = trimmed_word.cow_replace("*", "");

    let chars_trimmed_start = word.len() - start_trimmed_word.len();
    let chars_trimmed_end = start_trimmed_word.len() - trimmed_word.len();

    let end_pos = word.len() - chars_trimmed_end;

    (check_word, chars_trimmed_start..end_pos)
}

#[test]
fn test_trim_word_for_spellcheck() {
    assert_eq!(trim_word_for_spellcheck("word").0, "word");
    assert_eq!(trim_word_for_spellcheck("word").1, 0..4);

    assert_eq!(trim_word_for_spellcheck("word,").0, "word");
    assert_eq!(trim_word_for_spellcheck("word,").1, 0..4);

    assert_eq!(trim_word_for_spellcheck("*word*").0, "word");
    assert_eq!(trim_word_for_spellcheck("*word*").1, 1..5);

    assert_eq!(trim_word_for_spellcheck("*wo*rd").0, "word");
    assert_eq!(trim_word_for_spellcheck("*wo*rd").1, 1..6);
}

pub fn find_misspelled_words(text: &str, ctx: &EditorContext) -> Vec<(usize, usize)> {
    // Indexes of all of the misspelled words
    let mut misspelled_words = Vec::new();

    // we only spellcheck if we have a dictionary:
    if let Some(dict) = &ctx.dictionary_state.dictionary {
        // words in this case means everything that isn't whitespace, we'll take care of
        // trimming. We also split on `/` because those separate words without whitespace
        static WORD_REGEX: SavedRegex = SavedRegex::new(|| Regex::new(r"([^\s/]+)").unwrap());

        for word_match in WORD_REGEX.find_iter(text) {
            let (check_word, word_range) = trim_word_for_spellcheck(word_match.as_str());

            // floating punctuation isn't misspelled
            if !check_word.is_empty() && !dict.check(&check_word) {
                // We have a misspelled word now, compute boundaries

                let start_pos = word_match.start() + word_range.start;
                let end_pos = word_match.start() + word_range.end;

                assert!(start_pos < end_pos);

                // Check for the word that's currently being typed and
                // avoid adding it to the list of misspelled words. This delays
                // the detection a little bit, but I don't have a super nice way
                // of getting that to work
                if ctx.typing_status.is_new_word
                    && ctx.typing_status.current_word.contains(&start_pos)
                {
                    continue;
                }

                misspelled_words.push((start_pos, end_pos));
            }
        }
    }

    misspelled_words
}

pub fn word_count(text: &str) -> usize {
    static WORD_COUNT_REGEX: SavedRegex = SavedRegex::new(|| Regex::new(r"\s+").unwrap());
    let words: Vec<&str> = WORD_COUNT_REGEX.split(text).collect();

    if let Some(&last_word) = words.last() {
        if last_word.is_empty() {
            words.len() - 1
        } else {
            words.len()
        }
    } else {
        0
    }
}
