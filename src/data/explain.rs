use rusqlite::Connection;
use uuid::Uuid;

use crate::data::notes;

use super::{notes::NoteSearch, subjects::SubjectId, ConnectionType};

pub fn explain_all(ctype: ConnectionType) -> rusqlite::Result<()> {
    let store = super::Store::new(ctype);
    let conn = store.conn.borrow();

    let subject = SubjectId(Uuid::nil());

    let cases = [
        ("all notes", NoteSearch::new()),
        ("notes with subject", NoteSearch::new().subject(subject)),
        ("notes with task state", NoteSearch::new().task_only(true)),
        (
            "notes with subject and task state",
            NoteSearch::new().subject(subject).task_only(true),
        ),
    ];

    for (name, search) in cases.iter() {
        println!("Explain query plan for: {}", name);
        let query = notes::query_for_search(*search);
        print_query_plan(&conn, &query)?;
        println!();
    }

    Ok(())
}

fn print_query_plan(conn: &Connection, query: &str) -> rusqlite::Result<()> {
    use std::collections::BTreeMap;

    let mut levels = BTreeMap::new();
    levels.insert(0, 0);

    let mut parameters = Vec::new();
    if query.contains('?') {
        parameters.push("?");
    }

    let mut plan = conn.prepare(&format!("EXPLAIN QUERY PLAN {}", query))?;
    let mut rows = plan.query(rusqlite::params_from_iter(parameters))?;

    while let Some(row) = rows.next()? {
        let id = row.get_unwrap::<_, u32>(0);
        let parent = row.get_unwrap::<_, u32>(1);
        let detail = row.get_unwrap::<_, String>(3);
        let level = levels[&parent] + 1;
        levels.insert(id, level);

        let indent = "  ".repeat(level);
        println!("{}{}", indent, detail);
    }

    Ok(())
}
