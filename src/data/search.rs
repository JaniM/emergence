use rusqlite::Connection;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use tantivy::query::QueryParserError;
use tantivy::tokenizer::TextAnalyzer;
use tokio::sync::oneshot;

use crate::data::tfidf;

use super::functions::add_functions;
use super::notes;
use super::{
    notes::{Note, NoteData},
    ConnectionType,
};

#[derive(Clone)]
enum Query {
    Search(String),
    Similar(String),
}

struct Bridge<T> {
    inner: Arc<(Mutex<Option<T>>, Condvar)>,
}

impl<T> Clone for Bridge<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Bridge<T> {
    fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }
    fn send(&self, value: T) {
        let mut guard = self.inner.0.lock().unwrap();
        *guard = Some(value);
        self.inner.1.notify_one();
    }

    fn wait(&self) -> T {
        let mut guard = self
            .inner
            .1
            .wait_while(self.inner.0.lock().unwrap(), |v| v.is_none())
            .unwrap();
        guard.take().unwrap()
    }
}

struct SearchRequest {
    query: Query,
    send_data_to: oneshot::Sender<Vec<NoteData>>,
}

#[derive(Clone)]
pub struct SearchWorker {
    bridge: Bridge<SearchRequest>,
}

impl SearchWorker {
    pub fn start_search_thread(file: ConnectionType, index: Arc<Index>) -> SearchWorker {
        let conn = match file {
            ConnectionType::InMemory => Connection::open_in_memory().unwrap(),
            ConnectionType::File(path) => Connection::open(path).unwrap(),
        };
        add_functions(&conn).unwrap();

        let bridge = Bridge::<SearchRequest>::new();

        let reader = index.reader().unwrap();

        spawn_search(bridge.clone(), index, reader, conn);

        SearchWorker { bridge }
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
        self.bridge.send(query);

        let notes = receiver_to_main.await.unwrap();
        notes.into_iter().map(|n| n.to_note()).collect()
    }
}

fn spawn_search(
    bridge: Bridge<SearchRequest>,
    index: Arc<Index>,
    reader: IndexReader,
    conn: Connection,
) {
    // Note: the handler is not allowed to crash, so unwrap is strictly forbidden.
    let _handle = thread::spawn(move || loop {
        let request = bridge.wait();

        let result = match request.query {
            Query::Search(text) => search_text(&index, &reader, &conn, vec![text], 200),
            Query::Similar(text) => find_similar(&index, &reader, &conn, &text),
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
}

#[tracing::instrument(skip(index, reader, conn))]
fn search_text(
    index: &Index,
    reader: &IndexReader,
    conn: &Connection,
    texts: Vec<String>,
    limit: usize,
) -> rusqlite::Result<Vec<NoteData>> {
    use itertools::Itertools;

    tracing::trace!("Begin");

    let sanitized_texts = texts
        .iter()
        .map(|text| {
            text.to_lowercase()
                .replace(|c: char| !c.is_alphabetic(), " ")
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();

    let groups = sanitized_texts
        .iter()
        .map(|sanitized_text| format!("({})", sanitized_text))
        .join(" OR ");

    let notes = tantivy_find_notes(index, reader, conn, &groups, limit).unwrap();

    tracing::trace!("Found {} notes", notes.len());
    Ok(notes)
}

/// Find similar notes based on the TF-IDF algorithm.
fn find_similar(
    index: &Index,
    reader: &IndexReader,
    conn: &Connection,
    text: &str,
) -> rusqlite::Result<Vec<NoteData>> {
    let good_word_xount = 5;
    let best_words = tfidf::best_words(conn, text)?;

    let end_idx = std::cmp::min(best_words.len(), good_word_xount);
    if end_idx == 0 {
        return Ok(Vec::new());
    }

    let search = best_words[..end_idx].to_vec();

    tracing::debug!("Searching for: {}", search.join(" OR "));

    search_text(index, reader, conn, search, 20)
}

use tantivy::{schema::*, Index, IndexReader, TantivyError};

fn schema() -> Schema {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field(
        "text",
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("ngram3")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        ),
    );
    schema_builder.add_u64_field("id", INDEXED | STORED | FAST);

    schema_builder.build()
}

pub fn construct_tantivy_index(path: ConnectionType) -> Index {
    let schema = schema();
    let index = match path {
        ConnectionType::InMemory => Index::create_in_ram(schema.clone()),
        ConnectionType::File(path) => {
            let path = path.join("tantivy");
            std::fs::create_dir_all(&path).unwrap();
            let index = Index::create_in_dir(&path, schema.clone());
            match index {
                Ok(index) => index,
                Err(TantivyError::IndexAlreadyExists) => {
                    tracing::info!("Index already exists, opening it");
                    Index::open_in_dir(&path).unwrap()
                }
                Err(e) => panic!("Failed to create index: {}", e),
            }
        }
    };
    index.tokenizers().register(
        "ngram3",
        TextAnalyzer::builder(tantivy::tokenizer::NgramTokenizer::new(3, 3, false))
            .filter(tantivy::tokenizer::LowerCaser)
            .build(),
    );
    index
}

use tantivy::doc;

pub fn fill_tantivy_index(writer: &mut tantivy::IndexWriter, conn: &Connection) {
    writer.delete_all_documents().unwrap();
    writer.commit().unwrap();

    let mut stmt = conn
        .prepare_cached("SELECT rowid, text FROM notes")
        .unwrap();
    let mut rows = stmt.query([]).unwrap();

    let schema = schema();
    let id_schema = schema.get_field("id").unwrap();
    let text_schema = schema.get_field("text").unwrap();

    while let Some(row) = rows.next().unwrap() {
        let id: u64 = row.get(0).unwrap();
        let text: String = row.get(1).unwrap();
        let doc = doc!(
            id_schema => id,
            text_schema => text,
        );
        writer.add_document(doc).unwrap();
    }

    writer.commit().unwrap();
}

pub fn tantivy_add_note(writer: &mut tantivy::IndexWriter, note: &NoteData) -> tantivy::Result<()> {
    let schema = schema();
    let id_schema = schema.get_field("id").unwrap();
    let text_schema = schema.get_field("text").unwrap();

    let text = note.text.clone();
    let id = note.rowid as u64;

    let doc = doc!(
        id_schema => id,
        text_schema => text,
    );
    writer.add_document(doc).unwrap();
    writer.commit().unwrap();

    Ok(())
}

pub fn tantivy_remove_note(writer: &mut tantivy::IndexWriter, rowid: i64) -> tantivy::Result<()> {
    let schema = schema();
    let id_schema = schema.get_field("id").unwrap();

    writer.delete_term(Term::from_field_u64(id_schema, rowid as u64));
    writer.commit().unwrap();

    Ok(())
}

#[tracing::instrument(skip(index, reader, conn))]
fn tantivy_find_notes(
    index: &tantivy::Index,
    reader: &tantivy::IndexReader,
    conn: &Connection,
    text: &str,
    limit: usize,
) -> tantivy::Result<Vec<NoteData>> {
    if text.is_empty() {
        tracing::trace!("Ignoring empty search");
        return Ok(Vec::new());
    }

    let schema = schema();
    let id_schema = schema.get_field("id").unwrap();
    let text_schema = schema.get_field("text").unwrap();

    let searcher = reader.searcher();
    let query_parser = tantivy::query::QueryParser::for_index(index, vec![text_schema]);
    let query = query_parser.parse_query(text);

    let query = match query {
        Ok(query) => query,
        Err(QueryParserError::UnknownTokenizer { .. }) => {
            tracing::debug!("Unknown to tokenizer");
            return Ok(Vec::new());
        }
        Err(e) => {
            tracing::error!("Failed to parse query: {}", e);
            return Ok(Vec::new());
        }
    };

    let top_docs = searcher.search(&query, &tantivy::collector::TopDocs::with_limit(limit))?;

    let db_queey = format!(
        "SELECT {} FROM notes n WHERE rowid = ?",
        notes::SINGLE_NOTE_COLUMNS
    );
    let mut stmt = conn.prepare_cached(&db_queey).unwrap();

    tracing::trace!("Found {} results", top_docs.len());

    let mut notes = Vec::new();
    for (_score, doc_address) in top_docs {
        let retrieved_doc = searcher.doc(doc_address)?;
        let rowid = retrieved_doc
            .get_first(id_schema)
            .unwrap()
            .as_u64()
            .unwrap();
        let note = stmt.query_row([rowid], notes::map_row_to_note);
        let note = match note {
            Ok(note) => note,
            Err(_) => {
                continue;
            }
        };

        // SAFETY: We just created this note and the Rc is not shared
        // with anyone else. It is safe to unwrap.
        let note = std::rc::Rc::into_inner(note).unwrap();
        tracing::trace!("Found note: {:?}", note);
        notes.push(note);
    }

    Ok(notes)
}
