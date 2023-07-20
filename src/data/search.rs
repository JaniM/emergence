use rusqlite::Connection;
use std::thread;
use tokio::sync::mpsc::{error::TryRecvError, unbounded_channel, UnboundedSender};
use tokio::sync::oneshot;

use crate::data::tfidf;

use super::functions::add_functions;
use super::notes;
use super::{
    notes::{Note, NoteData},
    ConnectionType,
};

enum Query {
    Search(String),
    Similar(String),
}

struct SearchRequest {
    query: Query,
    send_data_to: oneshot::Sender<Vec<NoteData>>,
}

#[derive(Clone)]
pub struct SearchWorker {
    sender_to_worker: UnboundedSender<SearchRequest>,
}

impl SearchWorker {
    pub fn start_search_thread(file: ConnectionType) -> SearchWorker {
        let conn = match file {
            ConnectionType::InMemory => Connection::open_in_memory().unwrap(),
            ConnectionType::File(path) => Connection::open(path).unwrap(),
        };
        add_functions(&conn).unwrap();

        let (sender_to_worker, mut receiver_to_worker) = unbounded_channel::<SearchRequest>();

        // Note: the handler is not allowed to crash, so unwrap is strictly forbidden.
        let _handle = thread::spawn(move || loop {
            // Skip requests if we have a backlog.
            let mut current_request = None;
            let request = loop {
                match receiver_to_worker.try_recv() {
                    Ok(request) => current_request = Some(request),
                    Err(TryRecvError::Empty) => {
                        if let Some(request) = current_request {
                            break request;
                        }
                        current_request = match receiver_to_worker.blocking_recv() {
                            Some(request) => Some(request),
                            None => {
                                tracing::info!("Search channel is dead, exiting");
                                return;
                            }
                        }
                    }
                    Err(TryRecvError::Disconnected) => {
                        tracing::info!("Search channel is dead, exiting");
                        return;
                    }
                }
            };

            let result = match request.query {
                Query::Search(text) => search_text(&conn, vec![text], 50),
                Query::Similar(text) => find_similar(&conn, &text),
            };
            let result = match result {
                Ok(result) => result,
                Err(e) => {
                    // TODO: Tell the user that the search failed.
                    tracing::error!("Failed to search for text: {}", e);
                    Vec::new()
                }
            };

            let _send_result = request.send_data_to.send(result);
        });

        SearchWorker { sender_to_worker }
    }

    pub async fn perform_search(&self, search_text: String) -> Vec<Note> {
        self.perform(Query::Search(search_text)).await
    }

    pub async fn find_similar(&self, search_text: String) -> Vec<Note> {
        self.perform(Query::Similar(search_text)).await
    }

    async fn perform(&self, query: Query) -> Vec<Note> {
        let (sender_to_main, receiver_to_main) = oneshot::channel();
        let query = SearchRequest {
            query,
            send_data_to: sender_to_main,
        };
        self.sender_to_worker.send(query).unwrap();

        let notes = receiver_to_main.await.unwrap();
        notes.into_iter().map(|n| n.to_note()).collect()
    }
}

#[tracing::instrument(skip(conn))]
fn search_text(
    conn: &Connection,
    texts: Vec<String>,
    limit: usize,
) -> rusqlite::Result<Vec<NoteData>> {
    use itertools::Itertools;
    tracing::debug!("Begin");

    let sanitized_texts = texts
        .iter()
        .map(|text| {
            text.to_lowercase()
                .replace(|c: char| !c.is_alphabetic(), " ")
        })
        .collect::<Vec<_>>();

    let groups = sanitized_texts
        .iter()
        .map(|sanitized_text| {
            let trigrams = sanitized_text
                .split_ascii_whitespace()
                .flat_map(|w| {
                    w.chars()
                        .tuple_windows()
                        .map(|(a, b, c)| format!("{}{}{}", a, b, c))
                })
                .unique()
                .join(" AND ");
            format!("({})", trigrams)
        })
        .join(" OR ");

    let query = format!(
        "SELECT {columns}
        FROM notes_fts
        INNER JOIN notes n ON notes_fts.rowid = n.rowid
        WHERE notes_fts MATCH ?1",
        columns = notes::NOTE_COLUMNS,
    );
    let mut stmt = conn.prepare_cached(&query)?;
    let mut rows = stmt.query(rusqlite::params![groups])?;

    let mut notes = Vec::new();
    while let Some(row) = rows.next()? {
        let note = notes::map_row_to_note(row)?;
        // SAFETY: We just created this note and the Rc is not shared
        // with anyone else. It is safe to unwrap.
        let note = std::rc::Rc::into_inner(note).unwrap();

        // Note: There's no justification for this ranking algorithm.
        // It's just something I came up with that seems to work.

        let lower_note_text = note.text.to_lowercase();
        let mut matched = false;
        let mut rank = 0.0;
        'outer: for (group_idx, group) in sanitized_texts.iter().rev().enumerate() {
            let mut group_rank = 0.0;
            let mut prev_idx = 0;
            for word in group.split_whitespace() {
                let idx = lower_note_text.find(word);
                if let Some(idx) = idx {
                    group_rank += 1.0 / ((idx as f32 - prev_idx as f32).abs() + 1.0);
                    prev_idx = idx;
                } else {
                    continue 'outer;
                }
            }
            rank += group_rank * (group_idx as f32 + 1.0);
            matched = true;
        }
        if !matched {
            continue;
        }

        notes.push((rank, note));

        // Assume we have enough info for ranking after 100x the limit.
        if notes.len() >= limit * 100 {
            break;
        }
    }

    notes.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    tracing::debug!("Found {} notes", notes.len());
    let notes = notes
        .into_iter()
        .take(limit)
        .map(|(_, note)| note)
        .collect::<Vec<_>>();
    Ok(notes)
}

/// Find similar notes based on the TF-IDF algorithm.
fn find_similar(conn: &Connection, text: &str) -> rusqlite::Result<Vec<NoteData>> {
    let good_word_xount = 5;
    let best_words = tfidf::best_words(conn, text)?;

    let end_idx = std::cmp::min(best_words.len(), good_word_xount);
    if end_idx == 0 {
        return Ok(Vec::new());
    }

    let search = best_words[..end_idx].to_vec();

    tracing::debug!("Searching for: {}", search.join(" OR "));

    search_text(conn, search, 20)
}
