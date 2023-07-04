pub mod notes;
pub mod query;
pub mod subjects;

use rusqlite::{params, Connection, Result};
use std::cell::RefCell;
use std::rc::Rc;

use notes::NoteId;
use query::QuerySource;
use subjects::SubjectId;

pub struct Store {
    conn: rusqlite::Connection,
    sources: Rc<RefCell<Vec<Rc<RefCell<QuerySource>>>>>,
}

impl Store {
    pub fn new() -> Self {
        let conn = Connection::open("data.db").unwrap();
        setup_tables(&conn).unwrap();
        Self {
            conn,
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
        let id = self.conn.execute(
            "INSERT INTO notes (text, created_at) VALUES (?1, unixepoch(?2))",
            params![text, chrono::Utc::now()],
        )?;
        for subject in subjects {
            self.conn.execute(
                "INSERT INTO notes_subjects (note_id, subject_id) VALUES (?1, ?2)",
                params![id, subject.0],
            )?;
        }

        self.update_sources();

        Ok(NoteId(id as u64))
    }

    pub fn get_notes(&self) -> Result<Vec<notes::Note>> {
        println!("get_notes");
        let mut stmt = self.conn.prepare(
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
