use chrono::prelude::*;
use const_format::formatcp;
use rusqlite::{params, Connection, Row};
use std::rc::Rc;
use tracing::{instrument, trace};
use uuid::Uuid;

use super::{subjects::SubjectId, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NoteId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskState {
    NotATask,
    Todo,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NoteData {
    pub id: NoteId,
    pub text: String,
    pub subjects: Vec<SubjectId>,
    pub task_state: TaskState,
    pub created_at: DateTime<Local>,
    pub modified_at: DateTime<Local>,
}

pub type Note = Rc<NoteData>;

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
        Self { task_state, ..self.clone() }
    }
}

impl Store {
    #[instrument(skip(self))]
    pub fn add_note(&self, text: String, subjects: Vec<SubjectId>) -> rusqlite::Result<Note> {
        trace!("Adding note");
        let id = Uuid::new_v4();
        let created_at = Local::now();
        {
            let mut conn = self.conn.borrow_mut();
            let tx = conn.transaction()?;

            tx.prepare_cached(
                "INSERT INTO notes (id, text, created_at, modified_at)
                VALUES (?1, ?2, ?3, ?3)",
            )?
            .execute(params![id, text, created_at.naive_utc().timestamp_nanos()])?;

            for subject in &subjects {
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
            text,
            subjects,
            task_state: TaskState::NotATask,
            created_at,
            modified_at: created_at,
        }))
    }

    #[instrument(skip(self))]
    pub fn update_note(&self, note: Note) -> rusqlite::Result<()> {
        trace!("Updating note");
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
        trace!("Deleting note");
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
    pub fn get_notes(&self, subject: Option<SubjectId>) -> rusqlite::Result<Vec<Note>> {
        trace!("Begin");

        let conn = self.conn.borrow();
        let notes = if let Some(subject) = subject {
            notes_search_by_subject(&conn, subject)?
        } else {
            notes_list_all(&conn)?
        };

        trace!("Finished");
        Ok(notes)
    }
}

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
    r#"SELECT
        {}
    FROM notes n
    ORDER BY n.created_at DESC
    LIMIT 1000"#,
    NOTE_COLUMNS
);

const NOTE_SEARCH_BY_SUBJECT: &'static str = formatcp!(
    r#"SELECT
        {}
    FROM notes_search s
    INNER JOIN notes n ON s.note_id = n.id
    WHERE s.subject_id = ?1
    ORDER BY s.created_at DESC
    LIMIT 1000"#,
    NOTE_COLUMNS
);

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
