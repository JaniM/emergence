//! Implementation of the TF-IDF algorithm.
//! See https://en.wikipedia.org/wiki/Tf%E2%80%93idf
//!
//! Used to find the most relevant notes when entering a new note.

use std::collections::BTreeMap;

/// Trims punctuation from the beginning and end of a word.
/// Matches against the Alphabetic Unicode character property.
/// Returns an empty string if the word has no alphabetic characters.
fn trim_punctuation(word: &str) -> &str {
    let mut start = 0;
    let mut end = word.len();
    for (i, c) in word.char_indices() {
        if c.is_alphabetic() {
            start = i;
            break;
        }
    }
    for (i, c) in word.char_indices().rev() {
        if c.is_alphabetic() {
            end = i + c.len_utf8();
            break;
        }
    }
    &word[start..end]
}

fn naive_singularize(word: &str) -> &str {
    if word.ends_with('s') {
        &word[..word.len() - 1]
    } else {
        word
    }
}

fn ignore_code_blocks(text: &str) -> String {
    let mut result = String::new();
    let mut in_code_block = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
        }
        if !in_code_block {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

fn normalize_text(text: &str) -> String {
    let text = ignore_code_blocks(text);
    let text = text.replace(|c: char| !c.is_alphabetic(), " ");
    let text = text.to_lowercase();
    text
}

fn normalize_word(word: &str) -> &str {
    let word = trim_punctuation(word);
    let word = naive_singularize(word);
    let word = if word.ends_with("ing") && word.len() >= 6 {
        &word[..word.len() - 3]
    } else {
        word
    };
    word
}

/// Counts the number of times each word occurs in the text.
/// Returns a map from words to counts.
/// Words are trimmed of punctuation before counting.
/// Words are *not* normalized to lowercase.
fn count_word_occurrences(text: &str) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for word in text.split_whitespace() {
        let word = normalize_word(word);
        if word.len() < 3 {
            // We can't search for words shorter than 3 characters
            continue;
        }
        if word.len() > 30 {
            // Extremely long words are probably not real words
            continue;
        }

        *counts.entry(word).or_insert(0) += 1;
    }
    counts
}

pub fn best_words<'a, 'b>(
    conn: &'a rusqlite::Connection,
    text: &'b str,
) -> rusqlite::Result<Vec<String>> {
    use rusqlite::OptionalExtension;

    let text = normalize_text(text);

    let total_notes: usize = conn.query_row("SELECT COUNT(*) FROM notes;", [], |row| row.get(0))?;

    let mut stmt = conn.prepare_cached(
        "SELECT term, count
        FROM term_occurrences
        WHERE term = ?1;",
    )?;

    let counts = count_word_occurrences(&text);
    let word_xount = counts.len();
    let mut results = Vec::new();
    for (word, count_in_text) in counts {
        let term_frequency = count_in_text as f64 / word_xount as f64;
        let doc_count = stmt
            .query_row([word], |row| row.get::<_, i64>(1))
            .optional()?;

        // If the word is not in the database, we can skip it.
        let inverse_doc_frequency = match doc_count {
            Some(c) if c > 0 => (total_notes as f64 / c as f64).ln(),
            _ => continue,
        };

        let tfidf = term_frequency * inverse_doc_frequency;
        results.push((word, tfidf));
    }

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let best_words = results
        .into_iter()
        .map(|(word, _)| word.to_string())
        .collect::<Vec<_>>();

    Ok(best_words)
}

pub fn insert_word_occurences(conn: &rusqlite::Connection, text: &str) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare_cached(
        "INSERT INTO term_occurrences (term, count) VALUES (?1, ?2)
        ON CONFLICT(term) DO UPDATE SET count = count + excluded.count;",
    )?;

    let text = normalize_text(text);
    let counts = count_word_occurrences(&text);
    for (word, count) in counts {
        if count > 0 {
            stmt.execute((word, 1))?;
        }
    }

    Ok(())
}

pub fn remove_word_occurences(conn: &rusqlite::Connection, text: &str) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare_cached(
        "INSERT INTO term_occurrences (term, count) VALUES (?1, 0)
        ON CONFLICT(term) DO UPDATE SET count = count - 1;",
    )?;

    let text = normalize_text(text);
    let counts = count_word_occurrences(&text);
    for (word, count) in counts {
        if count > 0 {
            stmt.execute((word,))?;
        }
    }

    Ok(())
}

/// Fills the term_occurrences table with the word counts from the notes table.
/// This is used to perform a full reindex of the notes.
pub fn fill_word_occurence_table(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    tracing::info!("Filling word occurence table");
    let mut read_stmt = conn.prepare_cached("SELECT text FROM notes")?;

    // drop existing occurences
    conn.execute_batch("DELETE FROM term_occurrences;")?;

    let mut rows = read_stmt.query([])?;
    while let Some(row) = rows.next()? {
        let text: String = row.get(0)?;
        insert_word_occurences(conn, &text)?;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::data::{
        notes::{NoteBuilder, NoteData},
        ConnectionType, Store,
    };

    use super::*;

    #[test]
    fn test_best_word_solve() -> rusqlite::Result<()> {
        let store = Store::new(ConnectionType::InMemory);
        let note1 = store.add_note(NoteBuilder::new(
            "This has words we care about: xkcd".to_string(),
        ))?;
        store.add_note(NoteBuilder::new(
            "This does not have useful words".to_string(),
        ))?;

        {
            let conn = store.conn.borrow();
            let words = best_words(&conn, "xkcd foo word")?;
            assert_eq!(words, vec!["xkcd", "word"]);
        }

        store.update_note(
            NoteData {
                text: "This no longer has words we care about".to_string(),
                ..(&*note1).clone()
            }
            .to_note(),
        )?;

        {
            let conn = store.conn.borrow();
            let words = best_words(&conn, "xkcd foo word")?;
            assert_eq!(words, vec!["word"]);
        }

        Ok(())
    }
}
