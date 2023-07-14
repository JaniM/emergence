use std::rc::Rc;

use rusqlite::{params, ToSql};
use tracing::{debug, instrument};
use uuid::Uuid;

use super::{notes::NoteId, Store};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[repr(transparent)]
pub struct SubjectId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SubjectData {
    pub id: SubjectId,
    pub name: String,
}

pub type Subject = Rc<SubjectData>;

impl ToSql for SubjectId {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl std::fmt::Display for SubjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Store {
    #[instrument(skip(self))]
    pub fn get_subjects(&self) -> rusqlite::Result<Vec<Subject>> {
        debug!("Begin");
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare_cached(
            "SELECT id, name
            FROM subjects
            ORDER BY name ASC",
        )?;
        let subjects = stmt
            .query_map(params![], |row| {
                Ok(Rc::new(SubjectData {
                    id: SubjectId(row.get(0)?),
                    name: row.get(1)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        debug!("Finished");
        Ok(subjects)
    }

    #[instrument(skip(self))]
    pub fn add_subject(&mut self, name: String) -> rusqlite::Result<Subject> {
        debug!("Adding subject");
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

    pub fn delete_subject(&self, subject: SubjectId) -> rusqlite::Result<()> {
        self.conn
            .borrow()
            .prepare("DELETE FROM subjects WHERE id = ?1")?
            .execute(params![subject.0])?;

        self.update_subject_sources();

        Ok(())
    }

    pub fn import_subject(&self, subject: &SubjectData) -> rusqlite::Result<()> {
        self.conn
            .borrow()
            .prepare_cached("INSERT INTO subjects (id, name) VALUES (?1, ?2)")?
            .execute(params![subject.id.0, subject.name])?;
        Ok(())
    }

    pub fn get_notes_subjects(&self) -> rusqlite::Result<Vec<(NoteId, SubjectId)>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare_cached(
            "SELECT note_id, subject_id
            FROM notes_subjects",
        )?;
        let subjects = stmt
            .query_map(params![], |row| {
                Ok((NoteId(row.get(0)?), SubjectId(row.get(1)?)))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(subjects)
    }

    pub fn import_notes_subject(&self, note: NoteId, subject: SubjectId) -> rusqlite::Result<()> {
        self.conn
            .borrow()
            .prepare_cached("INSERT INTO notes_subjects (note_id, subject_id) VALUES (?1, ?2)")?
            .execute(params![note.0, subject.0])?;
        Ok(())
    }
}
