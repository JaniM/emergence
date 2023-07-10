mod functions;
pub mod notes;
pub mod query;
mod setup;
pub mod subjects;

use chrono::TimeZone;
use rusqlite::{params, Connection, Result, Row};
use std::rc::Rc;
use std::{cell::RefCell, collections::HashMap};
use tracing::{instrument, trace};
use uuid::Uuid;

use notes::NoteId;
use query::NoteQuerySource;
use subjects::SubjectId;

use self::query::SubjectQuerySource;
use self::subjects::{Subject, SubjectData};

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
    File(String),
}

impl Store {
    #[instrument()]
    pub fn new(file: ConnectionType) -> Self {
        trace!("Begin");
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

        trace!("Finished");
        store
    }

    #[instrument(skip_all)]
    pub(self) fn add_source(&self, source: Rc<RefCell<NoteQuerySource>>) {
        trace!("Adding note source");
        let subject = source.borrow().subject;
        source.borrow_mut().note_data = self.get_notes(subject).unwrap();
        self.note_sources.borrow_mut().push(source);
    }

    #[instrument(skip(self))]
    fn update_note_sources(&self) {
        let mut sources = self.note_sources.borrow_mut();
        sources.retain(|s| s.borrow().alive);
        for source in sources.iter() {
            let mut source = source.borrow_mut();
            source.note_data = self.get_notes(source.subject).unwrap();
            (source.update_callback)();
        }
    }

    #[instrument(skip(self))]
    fn update_subject_sources(&self) {
        let subjects = self.get_subjects().unwrap();
        let subjects = subjects
            .into_iter()
            .map(|s| (s.id, s))
            .collect::<HashMap<_, _>>();
        let mut source = self.subject_source.borrow_mut();
        source.subjects = subjects;
        for callback in source.update_callback.iter() {
            callback();
        }
    }

    #[instrument(skip(self))]
    pub fn add_note(&self, text: String, subjects: Vec<SubjectId>) -> Result<NoteId> {
        trace!("Adding note");
        let id = Uuid::new_v4();
        {
            let mut conn = self.conn.borrow_mut();
            let tx = conn.transaction()?;

            tx.prepare_cached(
                "INSERT INTO notes (id, text, created_at)
                VALUES (?1, ?2, unixepoch(?3))",
            )?
            .execute(params![id, text, chrono::Utc::now()])?;

            for subject in subjects {
                tx.prepare_cached(
                    "INSERT INTO notes_subjects (note_id, subject_id) VALUES (?1, ?2)",
                )?
                .execute(params![id, subject.0])?;
            }

            tx.commit()?;
        }

        self.update_note_sources();

        Ok(NoteId(id))
    }

    #[instrument(skip(self))]
    pub fn get_notes(&self, subject: Option<SubjectId>) -> Result<Vec<notes::Note>> {
        trace!("Begin");

        let row_f = |row: &Row| {
            let subjects_blob = row.get_ref(2)?.as_blob_or_null()?.unwrap_or_default();
            let subjects = subjects_blob
                .chunks_exact(16)
                .map(|chunk| SubjectId(Uuid::from_slice(chunk).unwrap()))
                .collect::<Vec<_>>();

            Ok(Rc::new(notes::NoteData {
                id: NoteId(row.get(0)?),
                text: row.get(1)?,
                subjects,
                created_at: chrono::Local.timestamp_opt(row.get(3)?, 0).unwrap(),
            }))
        };

        let conn = self.conn.borrow();
        let notes = if subject.is_none() {
            conn.prepare_cached(
                r#"SELECT
                    n.id,
                    n.text,
                    (SELECT concat_blobs(ns.subject_id) FROM notes_subjects ns WHERE ns.note_id = n.id)
                    as subjects,
                    n.created_at
                FROM notes n
                ORDER BY n.created_at DESC
                LIMIT 1000"#
            )?.query_map(params![], row_f)?
            .collect::<Result<Vec<_>, _>>()?
        } else {
            conn.prepare_cached(
                r#"SELECT
                    n.id,
                    n.text,
                    (SELECT concat_blobs(ns.subject_id) FROM notes_subjects ns WHERE ns.note_id = n.id)
                    as subjects,
                    n.created_at
                FROM notes_search s
                INNER JOIN notes n ON s.note_id = n.id
                WHERE s.subject_id = ?1
                ORDER BY s.created_at DESC
                LIMIT 1000"#
            )?.query_map(params![subject], row_f)?
            .collect::<Result<Vec<_>, _>>()?
        };

        trace!("Finished");
        Ok(notes)
    }

    /// Finds subjects that match the given search string.
    /// The search is case-insensitive and matches substrings.
    /// The results are sorted alphabetically.
    /// TODO: Implement a full-text search (FTS5).
    #[instrument(skip(self))]
    pub fn get_subjects(&self) -> Result<Vec<Subject>> {
        trace!("Begin");
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare_cached(
            "SELECT id, name
            FROM subjects
            ORDER BY name",
        )?;
        let subjects = stmt
            .query_map(params![], |row| {
                Ok(Rc::new(SubjectData {
                    id: SubjectId(row.get(0)?),
                    name: row.get(1)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        trace!("Finished");
        Ok(subjects)
    }

    #[instrument(skip(self))]
    pub fn add_subject(&mut self, name: String) -> Result<Subject> {
        trace!("Adding subject");
        let id = Uuid::new_v4();
        self.conn
            .borrow()
            .prepare_cached(
                "INSERT INTO subjects (id, name)
                VALUES (?1, ?2)",
            )?
            .execute(params![id, name])?;

        self.update_subject_sources();

        Ok(Rc::new(SubjectData {
            id: SubjectId(id),
            name,
        }))
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        trace!("Optimize database");
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
            "INSERT INTO notes (id, text, created_at)
            VALUES (?1, ?2, unixepoch(?3))",
        )?
        .execute(params![id, format!("Test Note {}", i), chrono::Utc::now()])?;
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
}
