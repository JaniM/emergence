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
                Query::Search(text) => search_text(&conn, &text),
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

/// Searches the database for text.
///
/// This implementation is very simple and inefficient.
/// In the 1 million note test case, i'm getting 2 second worst case
/// search times. This is not acceptable, but it's good enough for now.
#[tracing::instrument(skip(conn))]
fn search_text(conn: &Connection, text: &str) -> rusqlite::Result<Vec<NoteData>> {
    tracing::debug!("Begin");

    let query = format!(
        "SELECT {columns}
        FROM notes_fts
        INNER JOIN notes n ON notes_fts.rowid = n.rowid
        WHERE notes_fts MATCH ?1
        ORDER BY rank DESC
        LIMIT 50",
        columns = notes::NOTE_COLUMNS,
    );
    let mut stmt = conn.prepare_cached(&query)?;
    let mut rows = stmt.query(rusqlite::params![text])?;

    let mut notes = Vec::new();
    while let Some(row) = rows.next()? {
        let note = notes::map_row_to_note(row)?;
        // SAFETY: We just created this note and the Rc is not shared
        // with anyone else. It is safe to unwrap.
        notes.push(std::rc::Rc::into_inner(note).unwrap());
    }

    tracing::debug!("Found {} notes", notes.len());
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

    let search = best_words[..end_idx].join(" OR ");

    tracing::debug!("Searching for: {}", search);

    search_text(conn, &search)
}
