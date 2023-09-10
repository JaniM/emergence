use chrono::prelude::*;
use const_format::formatcp;
use rusqlite::{named_params, params, types::FromSql, Connection, Row, ToSql};
use std::rc::Rc;
use tracing::{debug, instrument, trace};
use uuid::Uuid;

use crate::data::{search, tfidf};

use super::{
    subjects::{subject_list_from_db, SubjectId},
    Store,
};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[repr(transparent)]
pub struct NoteId(pub Uuid);

impl ToSql for NoteId {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for NoteId {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Ok(NoteId(Uuid::column_result(value)?))
    }
}

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum TaskState {
    #[default]
    NotATask,
    Todo,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NoteData {
    pub rowid: i64,
    pub id: NoteId,
    pub text: String,
    pub subjects: Vec<SubjectId>,
    pub task_state: TaskState,
    pub created_at: DateTime<Local>,
    pub modified_at: DateTime<Local>,
    pub done_at: Option<DateTime<Local>>,
}

pub type Note = Rc<NoteData>;

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NoteBuilder {
    rowid: Option<i64>,
    pub id: Option<NoteId>,
    pub text: Option<String>,
    pub subjects: Option<Vec<SubjectId>>,
    pub task_state: Option<TaskState>,
    pub created_at: Option<DateTime<Local>>,
    pub modified_at: Option<DateTime<Local>>,
    pub done_at: Option<Option<DateTime<Local>>>,
}

impl NoteBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_id(mut self, id: NoteId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn text(mut self, text: impl ToString) -> Self {
        self.text = Some(text.to_string());
        self
    }

    pub fn subject(mut self, subject: SubjectId) -> Self {
        self.subjects.get_or_insert_with(Vec::new).push(subject);
        self
    }

    pub fn subjects(mut self, subjects: Vec<SubjectId>) -> Self {
        self.subjects.get_or_insert_with(Vec::new).extend(subjects);
        self
    }

    pub fn task_state(mut self, task_state: TaskState) -> Self {
        self.task_state = Some(task_state);
        self
    }

    pub fn created_at(mut self, time: DateTime<Local>) -> Self {
        self.created_at = Some(time);
        self
    }

    pub fn modified_at(mut self, time: DateTime<Local>) -> Self {
        self.modified_at = Some(time);
        self
    }

    pub fn done_at(mut self, time: Option<DateTime<Local>>) -> Self {
        self.done_at = Some(time);
        self
    }

    pub fn modified_now(self) -> Self {
        self.modified_at(Local::now())
    }

    pub fn apply_to_note_inplace(self, note: &mut NoteData) {
        note.rowid = self.rowid.unwrap_or(note.rowid);
        note.id = self.id.unwrap_or(note.id);
        if let Some(text) = self.text {
            note.text = text;
        }
        if let Some(subjects) = self.subjects {
            note.subjects = subjects;
        }
        note.task_state = self.task_state.unwrap_or(note.task_state);
        note.created_at = self.created_at.unwrap_or(note.created_at);
        note.modified_at = self.modified_at.unwrap_or(note.modified_at);
        note.done_at = self.done_at.unwrap_or(note.done_at);
    }

    pub fn apply_to_note(self, note: &NoteData) -> NoteData {
        let mut note = note.clone();
        self.apply_to_note_inplace(&mut note);
        note
    }

    pub fn build(self) -> NoteData {
        let now = Local::now();
        NoteData {
            rowid: self.rowid.unwrap_or_default(),
            id: self.id.unwrap_or_else(|| NoteId(Uuid::new_v4())),
            text: self.text.unwrap_or_default(),
            subjects: self.subjects.unwrap_or_default(),
            task_state: self.task_state.unwrap_or_default(),
            created_at: self.created_at.unwrap_or(now),
            modified_at: self.modified_at.unwrap_or(now),
            done_at: self.done_at.flatten(),
        }
    }
}

#[cfg(test)]
impl NoteBuilder {
    pub fn id(&self) -> NoteId {
        self.id.unwrap()
    }

    pub fn decide_id(mut self) -> Self {
        self.id = Some(NoteId(Uuid::new_v4()));
        self
    }

    pub fn assert_matches_note(&self, note: &NoteData) {
        let expected = self.clone().apply_to_note(note);
        assert_eq!(&expected, note);
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

impl ToSql for TaskState {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'static>> {
        Ok(rusqlite::types::ToSqlOutput::from(self.to_db_value()))
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

    pub(crate) fn to_builder(&self) -> NoteBuilder {
        NoteBuilder {
            rowid: Some(self.rowid),
            id: Some(self.id),
            text: Some(self.text.clone()),
            subjects: Some(self.subjects.clone()),
            task_state: Some(self.task_state),
            created_at: Some(self.created_at),
            modified_at: Some(self.modified_at),
            done_at: Some(self.done_at),
        }
    }

    pub fn modify_with(&self, f: impl FnOnce(NoteBuilder) -> NoteBuilder) -> NoteData {
        f(NoteBuilder::new()).apply_to_note(self)
    }
}

impl Store {
    #[instrument(skip(self))]
    pub fn add_note(&self, note: NoteBuilder) -> rusqlite::Result<Note> {
        debug!("Adding note");
        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction()?;
        let note = self.add_note_with_tx(&tx, note.build())?;
        tx.commit()?;
        Ok(note)
    }

    pub fn add_note_with_tx(&self, tx: &Connection, note: NoteData) -> rusqlite::Result<Note> {
        let mut note = note;
        tx.prepare_cached(
            "INSERT INTO notes (
                    id,
                    text,
                    task_state,
                    created_at,
                    modified_at,
                    done_at
                )
                VALUES (
                    :id,
                    :text,
                    :task_state,
                    :created_at,
                    :modified_at,
                    :done_at
                )",
        )?
        .execute(named_params! {
            ":id": &note.id,
            ":text": &note.text,
            ":task_state": &note.task_state,
            ":created_at": &note.created_at.naive_utc().timestamp_nanos(),
            ":modified_at": &note.modified_at.naive_utc().timestamp_nanos(),
            ":done_at": &note.done_at.map(|t| t.naive_utc().timestamp_nanos()),
        })?;

        note.rowid = tx.last_insert_rowid();

        let subjects = subjects_or_nil(&note.subjects);

        for subject in subjects {
            tx.prepare_cached("INSERT INTO notes_subjects (note_id, subject_id) VALUES (?1, ?2)")?
                .execute(params![note.id, subject.0])?;
        }

        tfidf::insert_word_occurences(tx, &note.text)?;

        let note = Rc::new(note);
        search::tantivy_add_note(&mut self.index_writer.borrow_mut(), &note).unwrap();

        Ok(note)
    }

    #[instrument(skip(self))]
    pub fn update_note(&self, note: NoteData) -> rusqlite::Result<()> {
        debug!("Updating note");
        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction()?;

        self.delete_note_with_tx(&tx, note.id)?;
        self.add_note_with_tx(&tx, note)?;

        tx.commit()?;

        Ok(())
    }

    #[instrument(skip(self))]
    pub fn delete_note(&self, note: NoteId) -> rusqlite::Result<()> {
        debug!("Deleting note");
        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction()?;

        self.delete_note_with_tx(&tx, note)?;

        tx.commit()?;

        Ok(())
    }

    fn delete_note_with_tx(&self, tx: &Connection, note: NoteId) -> rusqlite::Result<()> {
        let (rowid, old_text) = tx
            .prepare_cached(
                "SELECT rowid, text FROM notes
                    WHERE id = ?1",
            )?
            .query_row(params![note.0], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;

        tfidf::remove_word_occurences(tx, &old_text)?;
        search::tantivy_remove_note(&mut self.index_writer.borrow_mut(), rowid).unwrap();

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

        Ok(())
    }

    #[instrument(skip(self))]
    pub fn find_notes(&self, query: NoteSearch) -> rusqlite::Result<Vec<NoteId>> {
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

        // Assert notes are unique
        debug_assert_eq!(
            notes.len(),
            notes.iter().collect::<std::collections::HashSet<_>>().len()
        );

        debug!("Finished");
        Ok(notes)
    }

    pub fn get_note(&self, note: NoteId) -> rusqlite::Result<Note> {
        trace!("Getting note {}", note.0);
        let conn = self.conn.borrow();
        let notes = conn
            .prepare_cached(formatcp!(
                r#"SELECT {columns}
                    FROM notes n
                    WHERE n.id = ?1"#,
                columns = SINGLE_NOTE_COLUMNS
            ))?
            .query_row(params![note.0], map_row_to_note)?;
        Ok(notes)
    }

    pub fn get_notes(&self, notes: &[NoteId]) -> rusqlite::Result<Vec<Note>> {
        notes.iter().map(|note| self.get_note(*note)).collect()
    }

    pub fn get_all_notes(&self) -> rusqlite::Result<Vec<Note>> {
        let conn = self.conn.borrow();
        let notes = conn
            .prepare_cached(formatcp!(
                r#"SELECT {columns}
                    FROM notes n
                    ORDER BY n.created_at DESC"#,
                columns = SINGLE_NOTE_COLUMNS
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

pub const SINGLE_NOTE_COLUMNS: &str = "
    n.rowid,
    n.id,
    n.text,
    (SELECT concat_blobs(ns.subject_id) FROM notes_subjects ns WHERE ns.note_id = n.id)
    as subjects,
    n.task_state,
    n.created_at,
    n.modified_at,
    n.done_at
";

const NOTE_LIST_ALL: &str = formatcp!(
    r#"SELECT DISTINCT s.note_id
    FROM notes_search s
    ORDER BY s.created_at DESC
    LIMIT {page}"#,
    page = PAGE_SIZE
);

const NOTE_SEARCH_BY_SUBJECT: &str = formatcp!(
    r#"SELECT s.note_id
    FROM notes_search s
    WHERE s.subject_id = ?1
    ORDER BY s.created_at DESC
    LIMIT {page}"#,
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

fn notes_list_all(conn: &Connection) -> rusqlite::Result<Vec<NoteId>> {
    conn.prepare_cached(NOTE_LIST_ALL)?
        .query_map(params![], |row| row.get(0))?
        .collect()
}

fn notes_search_by_subject(conn: &Connection, subject: SubjectId) -> rusqlite::Result<Vec<NoteId>> {
    conn.prepare_cached(NOTE_SEARCH_BY_SUBJECT)?
        .query_map(params![subject], |row| row.get(0))?
        .collect()
}

fn tasks_search_by_subject(
    conn: &Connection,
    subject: Option<SubjectId>,
) -> rusqlite::Result<Vec<NoteId>> {
    let search = tasks_query(subject);
    let params1 = params![subject];
    let params2 = params![];
    let params = if subject.is_some() { params1 } else { params2 };
    conn.prepare_cached(&search)?
        .query_map(params, |row| row.get(0))?
        .collect()
}

fn tasks_query(subject: Option<SubjectId>) -> String {
    let search = format!(
        r#"SELECT DISTINCT notes_search.note_id
        FROM notes_search
        WHERE notes_search.task_state > 0 
        {subject_clause}
        ORDER BY notes_search.task_state ASC, notes_search.created_at DESC
        LIMIT {page}"#,
        page = PAGE_SIZE,
        subject_clause = if subject.is_some() {
            "AND notes_search.subject_id = ?1"
        } else {
            ""
        }
    );

    search
}

pub(super) fn map_row_to_note(row: &Row) -> rusqlite::Result<Note> {
    let subjects = subject_list_from_db(row, 3)?;

    Ok(Rc::new(NoteData {
        rowid: row.get(0)?,
        id: NoteId(row.get(1)?),
        text: row.get(2)?,
        subjects,
        task_state: TaskState::from_db_value(row.get(4)?),
        created_at: Local.timestamp_nanos(row.get(5)?),
        modified_at: Local.timestamp_nanos(row.get(6)?),
        done_at: row
            .get::<_, Option<i64>>(7)?
            .map(|ts| Local.timestamp_nanos(ts)),
    }))
}

fn subjects_or_nil(subjects: &[SubjectId]) -> &[SubjectId] {
    const NO_SUBJECT: [SubjectId; 1] = [SubjectId(Uuid::nil())];

    if subjects.is_empty() {
        &NO_SUBJECT
    } else {
        subjects
    }
}
