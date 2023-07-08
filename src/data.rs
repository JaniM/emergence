pub mod notes;
pub mod query;
pub mod subjects;

use chrono::TimeZone;
use rusqlite::functions::{Aggregate, Context, FunctionFlags};
use rusqlite::{params, Connection, Result};
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
    conn: Rc<RefCell<rusqlite::Connection>>,
    note_sources: Rc<RefCell<Vec<Rc<RefCell<NoteQuerySource>>>>>,
    subject_source: Rc<RefCell<SubjectQuerySource>>,
}

impl Store {
    #[instrument()]
    pub fn new() -> Self {
        trace!("Begin");
        let conn = Connection::open("data.db").unwrap();
        add_instr_lower(&conn).unwrap();
        add_concat_blobs(&conn).unwrap();
        setup_tables(&conn).unwrap();
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
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare_cached(
            r#"SELECT
                id,
                text,
                concat_blobs(subject_id) as subjects,
                created_at
            FROM notes
            LEFT JOIN notes_subjects ON notes.id = notes_subjects.note_id
            WHERE EXISTS (
                SELECT 1
                FROM notes_subjects
                WHERE note_id = notes.id AND subject_id = ?1
            ) OR ?1 IS NULL
            GROUP BY id
            ORDER BY created_at DESC
            LIMIT 1000"#,
        )?;
        let notes = stmt
            .query_map(params![subject], |row| {
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
            })?
            .collect::<Result<Vec<_>, _>>()?;

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
            name: name.to_string(),
        }))
    }
}

fn setup_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS subjects (
            id BLOB PRIMARY KEY,
            name TEXT NOT NULL
        ) STRICT;
        CREATE TABLE IF NOT EXISTS notes (
            id BLOB PRIMARY KEY,
            text TEXT NOT NULL,
            created_at INTEGER NOT NULL
        ) STRICT;
        CREATE TABLE IF NOT EXISTS notes_subjects (
            note_id BLOB NOT NULL,
            subject_id BLOB NOT NULL,
            PRIMARY KEY (note_id, subject_id),
            FOREIGN KEY (note_id) REFERENCES notes(id),
            FOREIGN KEY (subject_id) REFERENCES subjects(id)
        ) STRICT;
    "#,
    )
}

/// Adds a SQLite function that performs a case-insensitive substring search.
/// TODO: Currently unused, check later if it's needed.
fn add_instr_lower(conn: &Connection) -> Result<()> {
    fn instr_lower(haystack: &str, needle: &str) -> bool {
        haystack.to_lowercase().contains(&needle.to_lowercase())
    }

    conn.create_scalar_function(
        "instr_lower",
        2,
        FunctionFlags::SQLITE_UTF8
            | FunctionFlags::SQLITE_DETERMINISTIC
            | FunctionFlags::SQLITE_INNOCUOUS,
        |ctx| {
            let haystack = ctx.get::<String>(0)?;
            let needle = ctx.get::<String>(1)?;
            Ok(instr_lower(&haystack, &needle))
        },
    )
}

fn add_concat_blobs(conn: &Connection) -> Result<()> {
    struct ConcatBlobs;

    /// TODO: Avoid allocating a Vec for each row, maybe with ArrayVec.
    impl Aggregate<Vec<u8>, Vec<u8>> for ConcatBlobs {
        fn init(&self, _ctx: &mut Context<'_>) -> rusqlite::Result<Vec<u8>> {
            Ok(Vec::new())
        }

        fn step(&self, ctx: &mut Context<'_>, result: &mut Vec<u8>) -> rusqlite::Result<()> {
            let blob = ctx.get_raw(0).as_blob_or_null()?;
            if let Some(blob) = blob {
                result.extend_from_slice(blob);
            }
            Ok(())
        }

        fn finalize(&self, _: &mut Context<'_>, result: Option<Vec<u8>>) -> Result<Vec<u8>> {
            Ok(result.unwrap_or_default())
        }
    }

    conn.create_aggregate_function(
        "concat_blobs",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        ConcatBlobs,
    )
}

#[allow(dead_code)]
fn shove_test_data(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    let subject_ids = (1..=5)
        .map(|i| {
            let id = Uuid::new_v4();
            tx.prepare("INSERT INTO subjects (id, name) VALUES (?1, ?2)")?
                .execute(params![id, format!("Test Subject {}", i)])?;
            Ok(SubjectId(id))
        })
        .collect::<Result<Vec<_>>>()?;
    for i in 0..100000 {
        let id = Uuid::new_v4();
        tx.prepare(
            "INSERT INTO notes (id, text, created_at)
            VALUES (?1, ?2, unixepoch(?3))",
        )?
        .execute(params![id, format!("Test Note {}", i), chrono::Utc::now()])?;
        tx.execute(
            "INSERT INTO notes_subjects (note_id, subject_id) VALUES (?1, ?2)",
            params![id, subject_ids[i % 5]],
        )?;
    }
    tx.commit()?;
    Ok(())
}
