pub mod notes;
pub mod query;
pub mod subjects;

use rusqlite::{params, Connection, Result};
use std::cell::RefCell;
use std::rc::Rc;

use notes::NoteId;
use query::QuerySource;
use subjects::SubjectId;

use self::subjects::{Subject, SubjectData};

pub struct Store {
    conn: Rc<RefCell<rusqlite::Connection>>,
    sources: Rc<RefCell<Vec<Rc<RefCell<QuerySource>>>>>,
}

impl Store {
    pub fn new() -> Self {
        let conn = Connection::open("data.db").unwrap();
        setup_tables(&conn).unwrap();
        Self {
            conn: Rc::new(RefCell::new(conn)),
            sources: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub(self) fn add_source(&self, source: Rc<RefCell<QuerySource>>) {
        source.borrow_mut().note_data = self.get_notes().unwrap();
        self.sources.borrow_mut().push(source);
    }

    fn update_sources(&self) {
        let mut sources = self.sources.borrow_mut();
        sources.retain(|s| s.borrow().alive);
        for source in sources.iter() {
            let mut source = source.borrow_mut();
            source.note_data = self.get_notes().unwrap();
            (source.update_callback)();
        }
    }

    pub fn add_note(&self, text: String, subjects: Vec<SubjectId>) -> Result<NoteId> {
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

        self.update_sources();

        Ok(NoteId(id))
    }

    pub fn get_notes(&self) -> Result<Vec<notes::Note>> {
        println!("get_notes");
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare_cached(
            r#"SELECT
                id,
                text,
                coalesce(group_concat(subject_id), "") as subjects,
                datetime(created_at, 'unixepoch') as created_at
            FROM notes
            LEFT JOIN notes_subjects ON notes.id = notes_subjects.note_id
            GROUP BY id
            ORDER BY created_at DESC"#,
        )?;
        let notes = stmt
            .query_map(params![], |row| {
                let subjects_string = row.get::<_, String>(2)?;
                let subjects = if subjects_string.is_empty() {
                    vec![]
                } else {
                    subjects_string
                        .split(',')
                        .map(|s| SubjectId(s.parse().unwrap()))
                        .collect()
                };

                Ok(notes::NoteData {
                    id: NoteId(row.get(0)?),
                    text: row.get(1)?,
                    subjects,
                    created_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(notes.into_iter().map(Rc::new).collect())
    }

    pub fn find_subjects(&self, search: &str) -> Result<Vec<Subject>> {
        println!("find_subjects: {}", search);
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare_cached(
            "SELECT id, name
            FROM subjects
            WHERE instr(name, ?1) > 0
            ORDER BY name",
        )?;
        let subjects = stmt
            .query_map(params![search], |row| {
                Ok(Rc::new(SubjectData {
                    id: SubjectId(row.get(0)?),
                    name: row.get(1)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(subjects)
    }

    pub fn add_subject(&mut self, name: String) -> Result<Subject> {
        let id = self
            .conn
            .borrow()
            .prepare("INSERT INTO subjects (name) VALUES (?1)")?
            .query_row(params![name], |row| row.get(0))?;
        Ok(Rc::new(SubjectData {
            id: SubjectId(id),
            name: name.to_string(),
        }))
    }
}

fn setup_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS subjects (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY,
            text TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS notes_subjects (
            note_id INTEGER NOT NULL,
            subject_id INTEGER NOT NULL,
            PRIMARY KEY (note_id, subject_id),
            FOREIGN KEY (note_id) REFERENCES notes(id),
            FOREIGN KEY (subject_id) REFERENCES subjects(id)
        );
    ",
    )
}
