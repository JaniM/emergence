mod functions;
pub mod notes;
pub mod query;
mod setup;
pub mod subjects;

use rusqlite::{params, Connection, Result};
use std::path::PathBuf;
use std::rc::Rc;
use std::{cell::RefCell, collections::HashMap};
use tracing::{info, instrument, debug};
use uuid::Uuid;

use query::NoteQuerySource;
use subjects::SubjectId;

use self::query::SubjectQuerySource;

#[derive(Debug)]
pub struct Store {
    pub conn: Rc<RefCell<rusqlite::Connection>>,
    note_sources: Rc<RefCell<Vec<Rc<RefCell<NoteQuerySource>>>>>,
    subject_source: Rc<RefCell<SubjectQuerySource>>,
}

#[derive(Debug, Clone)]
pub enum ConnectionType {
    #[allow(dead_code)]
    InMemory,
    File(PathBuf),
}

impl Store {
    #[instrument()]
    pub fn new(file: ConnectionType) -> Self {
        debug!("Begin");
        let conn = match file {
            ConnectionType::InMemory => Connection::open_in_memory().unwrap(),
            ConnectionType::File(path) => Connection::open(path).unwrap(),
        };

        functions::add_functions(&conn).unwrap();
        setup::setup_tables(&conn).unwrap();

        let store = Self {
            conn: Rc::new(RefCell::new(conn)),
            note_sources: Rc::new(RefCell::new(Vec::new())),
            subject_source: Rc::new(RefCell::new(SubjectQuerySource {
                subjects: HashMap::new(),
                update_callback: Vec::new(),
            })),
        };

        // shove_test_data(&mut store.conn.borrow_mut()).unwrap();
        store.update_subject_sources();

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
pub fn shove_test_data(conn: &mut Connection) -> Result<()> {
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
    for i in 0..10_000 {
        let id = Uuid::new_v4();
        tx.prepare(
            "INSERT INTO notes (id, text, created_at, modified_at)
            VALUES (?1, ?2, ?3, ?3)",
        )?
        .execute(params![
            id,
            format!("Test Note {}", i),
            chrono::Local::now().naive_utc().timestamp_nanos()
        ])?;
        tx.execute(
            "INSERT INTO notes_subjects (note_id, subject_id) VALUES (?1, ?2)",
            params![id, subject_ids[i % subject_xount].0],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod test {
    use std::ops::Deref;

    use crate::data::notes::NoteData;

    use super::*;
    use rusqlite::Result;

    #[test]
    fn test_note_query_by_subject() -> Result<()> {
        let mut store = Store::new(ConnectionType::InMemory);
        let subject1 = store.add_subject("Test subject 1".to_string())?;
        let subject2 = store.add_subject("Test subject 2".to_string())?;

        store.add_note("Test note 1".to_string(), vec![subject1.id])?;
        store.add_note("Test note 2".to_string(), vec![subject2.id])?;

        let notes = store.get_notes(None).unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].text, "Test note 2");
        assert_eq!(notes[0].subjects, vec![subject2.id]);
        assert_eq!(notes[1].text, "Test note 1");
        assert_eq!(notes[1].subjects, vec![subject1.id]);

        let notes = store.get_notes(Some(subject1.id)).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "Test note 1");

        let notes = store.get_notes(Some(subject2.id)).unwrap();
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

        let note1 = store.add_note("Test note 1".to_string(), vec![subject1.id])?;
        let _note2 = store.add_note("Test note 2".to_string(), vec![subject1.id])?;

        let modified_note1 = NoteData {
            text: "Test note 1 modified".to_string(),
            subjects: vec![subject2.id],
            ..note1.deref().clone()
        }
        .to_note();

        store.update_note(modified_note1)?;

        let notes = store.get_notes(None).unwrap();
        assert_eq!(notes.len(), 2);

        assert_eq!(notes[0].text, "Test note 2");
        assert_eq!(notes[0].subjects, vec![subject1.id]);
        assert!(notes[0].modified_at == notes[0].created_at);

        assert_eq!(notes[1].text, "Test note 1 modified");
        assert!(notes[1].modified_at > notes[1].created_at);
        assert_eq!(notes[1].subjects, vec![subject2.id]);

        let notes = store.get_notes(Some(subject1.id)).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "Test note 2");

        let notes = store.get_notes(Some(subject2.id)).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "Test note 1 modified");

        Ok(())
    }

    #[test]
    fn test_delete_note() -> Result<()> {
        let mut store = Store::new(ConnectionType::InMemory);
        let subject1 = store.add_subject("Test subject 1".to_string())?;
        let subject2 = store.add_subject("Test subject 2".to_string())?;

        let note1 = store.add_note("Test note 1".to_string(), vec![subject1.id])?;
        let note2 = store.add_note("Test note 2".to_string(), vec![subject1.id])?;

        store.delete_note(note1.id)?;

        let notes = store.get_notes(None).unwrap();
        assert_eq!(notes.len(), 1);

        assert_eq!(notes[0].text, "Test note 2");
        assert_eq!(notes[0].subjects, vec![subject1.id]);
        assert!(notes[0].modified_at == notes[0].created_at);

        let notes = store.get_notes(Some(subject1.id)).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "Test note 2");

        let notes = store.get_notes(Some(subject2.id)).unwrap();
        assert_eq!(notes.len(), 0);

        store.delete_note(note2.id)?;

        let notes = store.get_notes(None).unwrap();
        assert_eq!(notes.len(), 0);

        Ok(())
    }
}
