pub mod notes;
pub mod query;
pub mod subjects;

use chrono::TimeZone;
use rusqlite::functions::FunctionFlags;
use rusqlite::{params, Connection, Result};
use std::rc::Rc;
use std::{cell::RefCell, collections::HashMap};
use tracing::{instrument, trace};

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
        setup_tables(&conn).unwrap();
        let store = Self {
            conn: Rc::new(RefCell::new(conn)),
            note_sources: Rc::new(RefCell::new(Vec::new())),
            subject_source: Rc::new(RefCell::new(SubjectQuerySource {
                subjects: HashMap::new(),
                update_callback: Vec::new(),
            })),
        };
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
        let id = {
            let mut conn = self.conn.borrow_mut();
            let tx = conn.transaction()?;

            let id: u64 = tx
                .prepare_cached(
                    "INSERT INTO notes (text, created_at)
                    VALUES (?1, unixepoch(?2))
                    RETURNING id",
                )?
                .query_row(params![text, chrono::Utc::now()], |row| row.get(0))?;

            for subject in subjects {
                tx.execute(
                    "INSERT INTO notes_subjects (note_id, subject_id) VALUES (?1, ?2)",
                    params![id, subject.0],
                )?;
            }

            tx.commit()?;
            id
        };

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
                coalesce(group_concat(subject_id), "") as subjects,
                created_at
            FROM notes
            LEFT JOIN notes_subjects ON notes.id = notes_subjects.note_id
            WHERE EXISTS (
                SELECT 1
                FROM notes_subjects
                WHERE note_id = notes.id AND subject_id = ?1
            ) OR ?1 IS NULL
            GROUP BY id
            ORDER BY created_at DESC"#,
        )?;
        let notes = stmt
            .query_map(params![subject], |row| {
                let subjects_string = row.get::<_, String>(2)?;
                let subjects = if subjects_string.is_empty() {
                    vec![]
                } else {
                    subjects_string
                        .split(',')
                        .map(|s| SubjectId(s.parse().unwrap()))
                        .collect()
                };

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
        let id = self
            .conn
            .borrow()
            .prepare(
                "INSERT INTO subjects (name)
                VALUES (?1)
                RETURNING id",
            )?
            .query_row(params![name], |row| row.get(0))?;

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
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        ) STRICT;
        CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY,
            text TEXT NOT NULL,
            created_at INTEGER NOT NULL
        ) STRICT;
        CREATE TABLE IF NOT EXISTS notes_subjects (
            note_id INTEGER NOT NULL,
            subject_id INTEGER NOT NULL,
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

#[allow(dead_code)]
fn shove_test_data(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    for i in 0..10000 {
        let id: u64 = tx
            .prepare(
                "INSERT INTO notes (text, created_at)
            VALUES (?1, unixepoch(?2))
            RETURNING id",
            )?
            .query_row(
                params![format!("Test Note {}", i), chrono::Utc::now()],
                |row| row.get(0),
            )?;
        tx.execute(
            "INSERT INTO notes_subjects (note_id, subject_id) VALUES (?1, ?2)",
            params![id, (i % 5) + 1],
        )?;
    }
    tx.commit()?;
    Ok(())
}
