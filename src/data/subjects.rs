use std::rc::Rc;

use rusqlite::{ToSql, params};
use tracing::{instrument, trace};
use uuid::Uuid;

use super::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SubjectId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
        trace!("Begin");
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
        trace!("Finished");
        Ok(subjects)
    }

    #[instrument(skip(self))]
    pub fn add_subject(&mut self, name: String) -> rusqlite::Result<Subject> {
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