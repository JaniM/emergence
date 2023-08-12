pub mod explain;
pub mod export;
mod functions;
pub mod notes;
pub mod search;
mod setup;
pub mod subjects;
pub mod tfidf;
pub mod layer;

use rusqlite::{params, Connection, Result};
use std::path::PathBuf;
use std::rc::Rc;
use std::{cell::RefCell, sync::Arc};
use tracing::{debug, info, instrument};
use uuid::Uuid;

use subjects::SubjectId;

pub struct Store {
    pub conn: Rc<RefCell<rusqlite::Connection>>,
    pub search: search::SearchWorker,

    index_writer: RefCell<tantivy::IndexWriter>,
}

#[derive(Debug, Clone)]
pub enum ConnectionType {
    #[allow(dead_code)]
    InMemory,
    File(PathBuf),
}

impl Store {
    #[instrument()]
    pub fn new(dir: ConnectionType) -> Self {
        debug!("Begin");
        let file = match dir.clone() {
            ConnectionType::InMemory => ConnectionType::InMemory,
            ConnectionType::File(path) => {
                let db_file = path.join("data.db");
                let _ = std::fs::create_dir_all(path);
                ConnectionType::File(db_file)
            }
        };

        let mut conn = match &file {
            ConnectionType::InMemory => Connection::open_in_memory().unwrap(),
            ConnectionType::File(path) => Connection::open(path).unwrap(),
        };

        functions::add_functions(&conn).unwrap();
        setup::setup_tables(&mut conn).unwrap();

        let index = search::construct_tantivy_index(dir);
        let index = Arc::new(index);
        let index_writer = index.writer(5_000_000).unwrap();
        let index_writer = RefCell::new(index_writer);

        let store = Self {
            conn: Rc::new(RefCell::new(conn)),
            search: search::SearchWorker::start_search_thread(file, index.clone()),
            index_writer,
        };

        debug!("Finished");
        store
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        info!("Optimize database");
        self.conn
            .borrow()
            .execute_batch(
                r#"
            pragma optimize;
            "#,
            )
            .unwrap();
    }
}

#[allow(dead_code)]
pub fn shove_test_data(conn: &mut Connection, count: usize) -> Result<()> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let tx = conn.transaction()?;
    let subject_xount = 10;
    let subject_ids = (1..=subject_xount)
        .map(|i| {
            let id = Uuid::new_v4();
            tx.prepare("INSERT INTO subjects (id, name) VALUES (?1, ?2)")?
                .execute(params![id, format!("Test Subject {}", i)])?;
            Ok(SubjectId(id))
        })
        .collect::<Result<Vec<_>>>()?;
    for i in 1..=count {
        let id = Uuid::new_v4();
        let task_state = i % 3;
        let word_count = rng.gen_range(5..=100);
        let text = lipsum::lipsum_words_with_rng(&mut rng, word_count);
        tx.prepare_cached(
            "INSERT INTO notes (id, text, task_state, created_at, modified_at)
            VALUES (?1, ?2, ?3, ?4, ?4)",
        )?
        .execute(params![
            id,
            text,
            task_state,
            chrono::Local::now().naive_utc().timestamp_nanos()
        ])?;
        tx.prepare_cached("INSERT INTO notes_subjects (note_id, subject_id) VALUES (?1, ?2)")?
            .execute(params![id, subject_ids[i % subject_xount].0])?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod test {
    use std::ops::Deref;

    use crate::data::notes::{NoteBuilder, NoteData, NoteSearch, TaskState};

    use super::*;
    use rusqlite::Result;

    #[test]
    fn test_note_query_by_subject() -> Result<()> {
        let mut store = Store::new(ConnectionType::InMemory);
        let subject1 = store.add_subject("Test subject 1".to_string())?;
        let subject2 = store.add_subject("Test subject 2".to_string())?;

        store.add_note(NoteBuilder::new("Test note 1".to_string()).subject(subject1.id))?;
        store.add_note(NoteBuilder::new("Test note 2".to_string()).subject(subject2.id))?;

        let note_ids = store.find_notes(NoteSearch::new()).unwrap();
        let notes = store.get_notes(&note_ids).unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].text, "Test note 2");
        assert_eq!(notes[0].subjects, vec![subject2.id]);
        assert_eq!(notes[1].text, "Test note 1");
        assert_eq!(notes[1].subjects, vec![subject1.id]);

        let note_ids = store
            .find_notes(NoteSearch::new().subject(subject1.id))
            .unwrap();
        let notes = store.get_notes(&note_ids).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "Test note 1");

        let note_ids = store
            .find_notes(NoteSearch::new().subject(subject2.id))
            .unwrap();
        let notes = store.get_notes(&note_ids).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "Test note 2");

        Ok(())
    }

    #[test]
    fn test_subject_query() -> Result<()> {
        let mut store = Store::new(ConnectionType::InMemory);
        store.add_subject("Test subject 1".to_string())?;
        store.add_subject("Test subject 2".to_string())?;

        let subjects = store.get_subjects()?;
        assert_eq!(subjects.len(), 2);
        assert_eq!(subjects[0].name, "Test subject 1");
        assert_eq!(subjects[1].name, "Test subject 2");

        Ok(())
    }

    #[test]
    #[ignore = "TODO"]
    fn cant_add_duplicate_subject() -> Result<()> {
        let mut store = Store::new(ConnectionType::InMemory);
        let name = "Test subject 1".to_string();
        store.add_subject(name.clone())?;
        assert!(store.add_subject(name).is_err());
        Ok(())
    }

    #[test]
    fn test_edit_note() -> Result<()> {
        let mut store = Store::new(ConnectionType::InMemory);
        let subject1 = store.add_subject("Test subject 1".to_string())?;
        let subject2 = store.add_subject("Test subject 2".to_string())?;

        let note1 =
            store.add_note(NoteBuilder::new("Test note 1".to_string()).subject(subject1.id))?;
        let _note2 =
            store.add_note(NoteBuilder::new("Test note 2".to_string()).subject(subject1.id))?;

        let modified_note1 = NoteData {
            text: "Test note 1 modified".to_string(),
            subjects: vec![subject2.id],
            ..note1.deref().clone()
        }
        .to_note();

        store.update_note(modified_note1)?;

        let note_ids = store.find_notes(NoteSearch::new()).unwrap();
        let notes = store.get_notes(&note_ids).unwrap();
        assert_eq!(notes.len(), 2);

        assert_eq!(notes[0].text, "Test note 2");
        assert_eq!(notes[0].subjects, vec![subject1.id]);
        assert!(notes[0].modified_at == notes[0].created_at);

        assert_eq!(notes[1].text, "Test note 1 modified");
        assert!(notes[1].modified_at > notes[1].created_at);
        assert_eq!(notes[1].subjects, vec![subject2.id]);

        let note_ids = store
            .find_notes(NoteSearch::new().subject(subject1.id))
            .unwrap();
        let notes = store.get_notes(&note_ids).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "Test note 2");

        let note_ids = store
            .find_notes(NoteSearch::new().subject(subject2.id))
            .unwrap();
        let notes = store.get_notes(&note_ids).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "Test note 1 modified");

        Ok(())
    }

    #[test]
    fn test_delete_note() -> Result<()> {
        let mut store = Store::new(ConnectionType::InMemory);
        let subject1 = store.add_subject("Test subject 1".to_string())?;
        let subject2 = store.add_subject("Test subject 2".to_string())?;

        let note1 =
            store.add_note(NoteBuilder::new("Test note 1".to_string()).subject(subject1.id))?;
        let note2 =
            store.add_note(NoteBuilder::new("Test note 2".to_string()).subject(subject1.id))?;

        store.delete_note(note1.id)?;

        let note_ids = store.find_notes(NoteSearch::new()).unwrap();
        let notes = store.get_notes(&note_ids).unwrap();
        assert_eq!(notes.len(), 1);

        assert_eq!(notes[0].text, "Test note 2");
        assert_eq!(notes[0].subjects, vec![subject1.id]);
        assert!(notes[0].modified_at == notes[0].created_at);

        let note_ids = store
            .find_notes(NoteSearch::new().subject(subject1.id))
            .unwrap();
        let notes = store.get_notes(&note_ids).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "Test note 2");

        let note_ids = store
            .find_notes(NoteSearch::new().subject(subject2.id))
            .unwrap();
        let notes = store.get_notes(&note_ids).unwrap();
        assert_eq!(notes.len(), 0);

        store.delete_note(note2.id)?;

        let notes = store.find_notes(NoteSearch::new()).unwrap();
        assert_eq!(notes.len(), 0);

        Ok(())
    }

    #[test]
    fn test_tasks() -> Result<()> {
        let mut store = Store::new(ConnectionType::InMemory);
        let subject1 = store.add_subject("Test subject 1".to_string())?;
        let subject2 = store.add_subject("Test subject 2".to_string())?;

        let note1 = store.add_note(
            NoteBuilder::new("Test note 1".to_string())
                .subject(subject1.id)
                .task_state(TaskState::NotATask),
        )?;

        let search = NoteSearch::new().task_only(true);

        let notes = store.find_notes(search).unwrap();
        assert_eq!(notes.len(), 0);

        store.update_note(note1.with_task_state(TaskState::Todo).to_note())?;

        let notes = store.find_notes(search).unwrap();
        assert_eq!(notes.len(), 1);

        let notes = store.find_notes(search.subject(subject1.id)).unwrap();
        assert_eq!(notes.len(), 1);
        let notes = store.find_notes(search.subject(subject2.id)).unwrap();
        assert_eq!(notes.len(), 0);

        store.update_note(note1.with_task_state(TaskState::Done).to_note())?;
        let notes = store.find_notes(search).unwrap();
        assert_eq!(notes.len(), 1);

        store.update_note(
            note1
                .with_task_state(TaskState::Todo)
                .with_subjects(vec![subject2.id])
                .to_note(),
        )?;

        let notes = store.find_notes(search).unwrap();
        assert_eq!(notes.len(), 1);

        let notes = store.find_notes(search.subject(subject1.id)).unwrap();
        assert_eq!(notes.len(), 0);
        let notes = store.find_notes(search.subject(subject2.id)).unwrap();
        assert_eq!(notes.len(), 1);

        Ok(())
    }

    #[test]
    fn test_delete_subject() -> Result<()> {
        let mut store = Store::new(ConnectionType::InMemory);
        let subject1 = store.add_subject("Test subject 1".to_string())?;
        let subject2 = store.add_subject("Test subject 2".to_string())?;

        let note1 =
            store.add_note(NoteBuilder::new("Test note 1".to_string()).subject(subject1.id))?;

        assert!(store.delete_subject(subject2.id).is_ok());

        store.delete_note(note1.id)?;

        assert!(store.delete_subject(subject1.id).is_ok());

        assert!(store.get_subjects()?.is_empty());

        Ok(())
    }
}
