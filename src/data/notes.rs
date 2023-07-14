use chrono::prelude::*;
use const_format::formatcp;
use rusqlite::{params, Connection, Row};
use std::rc::Rc;
use tracing::{debug, instrument};
use uuid::Uuid;

use super::{subjects::SubjectId, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
pub struct NoteId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TaskState {
    NotATask,
    Todo,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NoteData {
    pub id: NoteId,
    pub text: String,
    pub subjects: Vec<SubjectId>,
    pub task_state: TaskState,
    pub created_at: DateTime<Local>,
    pub modified_at: DateTime<Local>,
}

pub type Note = Rc<NoteData>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NoteBuilder {
    text: String,
    subjects: Vec<SubjectId>,
    task_state: TaskState,
}

impl NoteBuilder {
    pub fn new(text: String) -> Self {
        Self {
            text,
            subjects: Vec::new(),
            task_state: TaskState::NotATask,
        }
    }

    pub fn subject(mut self, subject: SubjectId) -> Self {
        self.subjects.push(subject);
        self
    }

    pub fn subjects(mut self, subjects: Vec<SubjectId>) -> Self {
        self.subjects.extend(subjects);
        self
    }

    pub fn task_state(mut self, task_state: TaskState) -> Self {
        self.task_state = task_state;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct NoteSearch {
    pub subject_id: Option<SubjectId>,
    pub task_only: bool,
}

impl NoteSearch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subject(self, subject_id: SubjectId) -> Self {
        Self {
            subject_id: Some(subject_id),
            ..self
        }
    }

    pub fn subject_opt(self, subject_id: Option<SubjectId>) -> Self {
        Self { subject_id, ..self }
    }

    pub fn task_only(self, task_only: bool) -> Self {
        Self { task_only, ..self }
    }
}

impl TaskState {
    pub fn to_db_value(self) -> i64 {
        match self {
            TaskState::NotATask => 0,
            TaskState::Todo => 1,
            TaskState::Done => 2,
        }
    }

    pub fn from_db_value(value: i64) -> Self {
        match value {
            0 => TaskState::NotATask,
            1 => TaskState::Todo,
            2 => TaskState::Done,
            _ => panic!("Invalid task state: {}", value),
        }
    }
}

impl NoteData {
    pub fn to_note(self) -> Note {
        Rc::new(self)
    }

    pub fn with_task_state(&self, task_state: TaskState) -> Self {
        Self {
            task_state,
            ..self.clone()
        }
    }

    #[cfg(test)]
    pub fn with_subjects(&self, subjects: Vec<SubjectId>) -> Self {
        Self {
            subjects,
            ..self.clone()
        }
    }
}

impl Store {
    #[instrument(skip(self))]
    pub fn add_note(&self, note: NoteBuilder) -> rusqlite::Result<Note> {
        debug!("Adding note");
        let id = Uuid::new_v4();
        let created_at = Local::now();
        {
            let mut conn = self.conn.borrow_mut();
            let tx = conn.transaction()?;

            tx.prepare_cached(
                "INSERT INTO notes (
                    id, text, task_state, created_at, modified_at
                )
                VALUES (?1, ?2, ?3, ?4, ?4)",
            )?
            .execute(params![
                id,
                &note.text,
                note.task_state.to_db_value(),
                created_at.naive_utc().timestamp_nanos()
            ])?;

            for subject in &note.subjects {
                tx.prepare_cached(
                    "INSERT INTO notes_subjects (note_id, subject_id) VALUES (?1, ?2)",
                )?
                .execute(params![id, subject.0])?;
            }

            tx.commit()?;
        }

        self.update_note_sources();

        Ok(Rc::new(NoteData {
            id: NoteId(id),
            text: note.text,
            subjects: note.subjects,
            task_state: TaskState::NotATask,
            created_at,
            modified_at: created_at,
        }))
    }

    pub fn import_note(&self, note: &NoteData) -> rusqlite::Result<()> {
        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction()?;
        tx.prepare_cached(
            "INSERT INTO notes (
                    id, text, task_state, created_at, modified_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5)",
        )?
        .execute(params![
            note.id.0,
            &note.text,
            note.task_state.to_db_value(),
            note.created_at.naive_utc().timestamp_nanos(),
            note.modified_at.naive_utc().timestamp_nanos()
        ])?;

        for subject in &note.subjects {
            tx.prepare_cached("INSERT INTO notes_subjects (note_id, subject_id) VALUES (?1, ?2)")?
                .execute(params![note.id.0, subject.0])?;
        }

        tx.commit()?;

        Ok(())
    }

    #[instrument(skip(self))]
    pub fn update_note(&self, note: Note) -> rusqlite::Result<()> {
        debug!("Updating note");
        {
            let mut conn = self.conn.borrow_mut();
            let tx = conn.transaction()?;

            tx.prepare_cached(
                "UPDATE notes
                SET text = ?2,
                    modified_at = ?3,
                    task_state = ?4
                WHERE id = ?1",
            )?
            .execute(params![
                note.id.0,
                note.text,
                Local::now().naive_local().timestamp_nanos(),
                note.task_state.to_db_value()
            ])?;

            tx.prepare_cached(
                "DELETE FROM notes_subjects
                WHERE note_id = ?1",
            )?
            .execute(params![note.id.0])?;

            for subject in &note.subjects {
                tx.prepare_cached(
                    "INSERT INTO notes_subjects (note_id, subject_id) VALUES (?1, ?2)",
                )?
                .execute(params![note.id.0, subject.0])?;
            }

            tx.commit()?;
        }

        self.update_note_sources();

        Ok(())
    }

    #[instrument(skip(self))]
    pub fn delete_note(&self, note: NoteId) -> rusqlite::Result<()> {
        debug!("Deleting note");
        {
            let mut conn = self.conn.borrow_mut();
            let tx = conn.transaction()?;

            tx.prepare_cached(
                "DELETE FROM notes_subjects
                WHERE note_id = ?1",
            )?
            .execute(params![note.0])?;

            tx.prepare_cached(
                "DELETE FROM notes
                WHERE id = ?1",
            )?
            .execute(params![note.0])?;

            tx.commit()?;
        }

        self.update_note_sources();

        Ok(())
    }

    #[instrument(skip(self))]
    pub fn get_notes(&self, query: NoteSearch) -> rusqlite::Result<Vec<Note>> {
        debug!("Begin");

        let conn = self.conn.borrow();
        let notes = match query {
            NoteSearch {
                subject_id: subject,
                task_only: true,
            } => tasks_search_by_subject(&conn, subject)?,
            NoteSearch {
                subject_id: Some(subject),
                task_only: false,
            } => notes_search_by_subject(&conn, subject)?,
            NoteSearch {
                subject_id: None,
                task_only: false,
            } => notes_list_all(&conn)?,
        };

        debug!("Finished");
        Ok(notes)
    }

    pub fn get_all_notes(&self) -> rusqlite::Result<Vec<Note>> {
        let conn = self.conn.borrow();
        let notes = conn
            .prepare_cached(formatcp!(
                r#"SELECT {columns}
                    FROM notes n
                    ORDER BY n.created_at DESC"#,
                columns = NOTE_COLUMNS
            ))?
            .query_map(params![], map_row_to_note)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(notes)
    }

    pub fn subject_note_count(&self, subject: SubjectId) -> rusqlite::Result<u64> {
        let conn = self.conn.borrow();
        let count = conn
            .prepare_cached(
                "SELECT COUNT(*) FROM notes_search
                WHERE subject_id = ?1",
            )?
            .query_row(params![subject], |row| row.get::<_, u64>(0))?;
        Ok(count)
    }
}

const PAGE_SIZE: usize = 200;

const NOTE_COLUMNS: &'static str = "
    n.id,
    n.text,
    (SELECT concat_blobs(ns.subject_id) FROM notes_subjects ns WHERE ns.note_id = n.id)
    as subjects,
    n.task_state,
    n.created_at,
    n.modified_at
";

const NOTE_LIST_ALL: &'static str = formatcp!(
    r#"SELECT {columns}
    FROM notes n
    ORDER BY n.created_at DESC
    LIMIT {page}"#,
    columns = NOTE_COLUMNS,
    page = PAGE_SIZE
);

const NOTE_SEARCH_BY_SUBJECT: &'static str = formatcp!(
    r#"SELECT {columns}
    FROM notes_search s
    INNER JOIN notes n ON s.note_id = n.id
    WHERE s.subject_id = ?1
    ORDER BY s.created_at DESC
    LIMIT {page}"#,
    columns = NOTE_COLUMNS,
    page = PAGE_SIZE
);

pub fn query_for_search(query: NoteSearch) -> String {
    match query {
        NoteSearch {
            subject_id,
            task_only: true,
        } => tasks_query(subject_id),
        NoteSearch {
            subject_id: Some(_),
            task_only: false,
        } => NOTE_SEARCH_BY_SUBJECT.to_owned(),
        NoteSearch {
            subject_id: None,
            task_only: false,
        } => NOTE_LIST_ALL.to_owned(),
    }
}

fn notes_list_all(conn: &Connection) -> rusqlite::Result<Vec<Note>> {
    conn.prepare_cached(NOTE_LIST_ALL)?
        .query_map(params![], map_row_to_note)?
        .collect()
}

fn notes_search_by_subject(conn: &Connection, subject: SubjectId) -> rusqlite::Result<Vec<Note>> {
    conn.prepare_cached(NOTE_SEARCH_BY_SUBJECT)?
        .query_map(params![subject], map_row_to_note)?
        .collect()
}

fn tasks_search_by_subject(
    conn: &Connection,
    subject: Option<SubjectId>,
) -> rusqlite::Result<Vec<Note>> {
    let search = tasks_query(subject);
    let params1 = params![subject];
    let params2 = params![];
    let params = if subject.is_some() { params1 } else { params2 };
    conn.prepare_cached(&search)?
        .query_map(params, map_row_to_note)?
        .collect()
}

fn tasks_query(subject: Option<SubjectId>) -> String {
    let search = format!(
        r#"SELECT {columns}
        FROM notes n
        {subject_clause}
        WHERE notes_search.task_state > 0 
        ORDER BY notes_search.task_state ASC, notes_search.created_at DESC
        LIMIT {page}"#,
        columns = NOTE_COLUMNS,
        page = PAGE_SIZE,
        subject_clause = if subject.is_some() {
            "INNER JOIN notes_search 
            ON notes_search.note_id = n.id AND notes_search.subject_id = ?1"
        } else {
            ""
        }
    );

    let search = if subject.is_some() {
        search
    } else {
        search.replace("notes_search", "n")
    };
    search
}

fn map_row_to_note(row: &Row) -> rusqlite::Result<Note> {
    let subjects_blob = row.get_ref(2)?.as_blob_or_null()?.unwrap_or_default();
    let subjects = subjects_blob
        .chunks_exact(16)
        .map(|chunk| SubjectId(Uuid::from_slice(chunk).unwrap()))
        .collect();

    Ok(Rc::new(NoteData {
        id: NoteId(row.get(0)?),
        text: row.get(1)?,
        subjects,
        task_state: TaskState::from_db_value(row.get(3)?),
        created_at: Local.timestamp_nanos(row.get(4)?),
        modified_at: Local.timestamp_nanos(row.get(5)?),
    }))
}
