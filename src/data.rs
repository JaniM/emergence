pub mod notes;
pub mod query;
pub mod subjects;

use chrono::TimeZone;
use rusqlite::{params, Connection, Result, ToSql};
use smallvec::SmallVec;
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
        let mut stmt = conn.prepare_cached(if subject.is_none() {
            r#"SELECT
                id,
                text,
                (SELECT concat_blobs(subject_id) FROM notes_subjects WHERE note_id = notes.id)
                as subjects,
                created_at
            FROM notes
            WHERE ?1 IS NULL
            ORDER BY created_at DESC
            LIMIT 1000"#
        } else {
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
        })?;
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
        CREATE INDEX IF NOT EXISTS notes_created_at ON notes (created_at);
        CREATE INDEX IF NOT EXISTS notes_subjects_index ON notes_subjects(subject_id, note_id);

        CREATE TABLE IF NOT EXISTS notes_search (
            note_id BLOB NOT NULL,
            subject_id BLOB NOT NULL,
            created_at INTEGER NOT NULL,
            PRIMARY KEY (note_id, subject_id),
            FOREIGN KEY (note_id) REFERENCES notes(id),
            FOREIGN KEY (subject_id) REFERENCES subjects(id)
        ) STRICT;

        CREATE INDEX IF NOT EXISTS notes_search_index ON notes_search (subject_id, created_at);

        CREATE TRIGGER IF NOT EXISTS notes_search_insert AFTER INSERT ON notes_subjects BEGIN
            INSERT INTO notes_search (note_id, subject_id, created_at)
            VALUES (
                NEW.note_id,
                NEW.subject_id,
                (SELECT created_at FROM notes WHERE id = NEW.note_id)
            );
        END;

        CREATE TRIGGER IF NOT EXISTS notes_search_delete AFTER DELETE ON notes_subjects BEGIN
            DELETE FROM notes_search
            WHERE note_id = OLD.note_id AND subject_id = OLD.subject_id;
        END;
    "#,
    )?;

    let search_index_count = conn
        .prepare_cached("SELECT COUNT(*) FROM notes_search")?
        .query_row(params![], |row| row.get::<_, i64>(0))?;

    if search_index_count == 0 {
        conn.execute_batch(
            r#"
            INSERT INTO notes_search (note_id, subject_id, created_at)
            SELECT note_id, subject_id, (SELECT created_at FROM notes WHERE id = note_id)
            FROM notes_subjects
            WHERE TRUE
            ON CONFLICT (note_id, subject_id) DO NOTHING;
        "#,
        )?;
    }
    Ok(())
}

fn add_concat_blobs(conn: &Connection) -> Result<()> {
    use rusqlite::functions::{Aggregate, Context, FunctionFlags};
    struct ConcatBlobs;

    /// Wrapper around SmallVec to implement ToSql.
    /// 64 bytes is enough for 4 UUIDs, which I assume is enough for most notes.
    #[derive(Default)]
    struct Blob(SmallVec<[u8; 64]>);

    impl ToSql for Blob {
        fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
            self.0.to_sql()
        }
    }

    /// TODO: Avoid allocating a Vec for each row, maybe with ArrayVec.
    impl Aggregate<Blob, Blob> for ConcatBlobs {
        fn init(&self, _ctx: &mut Context<'_>) -> rusqlite::Result<Blob> {
            Ok(Blob::default())
        }

        fn step(&self, ctx: &mut Context<'_>, result: &mut Blob) -> rusqlite::Result<()> {
            let blob = ctx.get_raw(0).as_blob_or_null()?;
            if let Some(blob) = blob {
                result.0.extend_from_slice(blob);
            }
            Ok(())
        }

        fn finalize(&self, _: &mut Context<'_>, result: Option<Blob>) -> Result<Blob> {
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
    let subject_xount = 100;
    let subject_ids = (1..=subject_xount)
        .map(|i| {
            let id = Uuid::new_v4();
            tx.prepare("INSERT INTO subjects (id, name) VALUES (?1, ?2)")?
                .execute(params![id, format!("Test Subject {}", i)])?;
            Ok(SubjectId(id))
        })
        .collect::<Result<Vec<_>>>()?;
    for i in 0..1_000_000 {
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
