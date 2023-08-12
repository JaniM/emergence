use rusqlite::{params, Connection, Result};

use super::tfidf;

pub fn setup_tables(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS subjects (
            id BLOB PRIMARY KEY,
            name TEXT NOT NULL,
            parent_id BLOB,
            UNIQUE (name, parent_id)
        ) WITHOUT ROWID, STRICT;

        CREATE TABLE IF NOT EXISTS notes (
            rowid INTEGER PRIMARY KEY AUTOINCREMENT,
            id BLOB,
            text TEXT NOT NULL,
            -- 0 = not a task, 1 = incomplete, 2 = complete
            task_state INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            modified_at INTEGER NOT NULL,
            done_at INTEGER
        ) STRICT;

        CREATE UNIQUE INDEX IF NOT EXISTS notes_id_index ON notes (id);


        CREATE TABLE IF NOT EXISTS notes_subjects (
            note_id BLOB NOT NULL,
            subject_id BLOB NOT NULL,
            PRIMARY KEY (note_id, subject_id)
        ) WITHOUT ROWID, STRICT;

        CREATE TABLE IF NOT EXISTS notes_search (
            rowid INTEGER PRIMARY KEY AUTOINCREMENT,
            note_id BLOB NOT NULL,
            subject_id BLOB NOT NULL,
            created_at INTEGER NOT NULL,
            task_state INTEGER NOT NULL DEFAULT 0
        ) STRICT;

        CREATE INDEX IF NOT EXISTS notes_search_created_at_index
            ON notes_search (created_at);
        CREATE INDEX IF NOT EXISTS notes_search_subject_index
            ON notes_search (subject_id, created_at);
        CREATE INDEX IF NOT EXISTS notes_search_tasks_index
            ON notes_search (task_state ASC, created_at DESC);
        CREATE INDEX IF NOT EXISTS notes_search_subject_tasks_index
            ON notes_search (subject_id, task_state ASC, created_at DESC);


        CREATE TRIGGER IF NOT EXISTS notes_search_insert AFTER INSERT ON notes_subjects BEGIN
            INSERT INTO notes_search (
                note_id, subject_id, task_state, created_at)
            VALUES (
                NEW.note_id,
                NEW.subject_id,
                (SELECT task_state FROM notes WHERE id = NEW.note_id),
                (SELECT created_at FROM notes WHERE id = NEW.note_id)
            );
        END;

        CREATE TRIGGER IF NOT EXISTS notes_search_delete AFTER DELETE ON notes_subjects BEGIN
            DELETE FROM notes_search
            WHERE note_id = OLD.note_id
            AND subject_id = OLD.subject_id;
        END;

        CREATE TABLE IF NOT EXISTS term_occurrences (
            term TEXT PRIMARY KEY,
            count INTEGER NOT NULL
        ) WITHOUT ROWID, STRICT;
    "#,
    )?;

    let search_index_count = conn
        .prepare_cached("SELECT COUNT(*) FROM notes_search")?
        .query_row(params![], |row| row.get::<_, i64>(0))?;

    if search_index_count == 0 {
        conn.execute_batch(
            r#"
            INSERT INTO notes_search (
                note_id, subject_id, task_state, created_at)
            SELECT
                note_id,
                subject_id,
                (SELECT task_state FROM notes WHERE id = note_id),
                (SELECT created_at FROM notes WHERE id = note_id)
            FROM notes_subjects;
        "#,
        )?;
    }

    let text_occurences_count = conn
        .prepare_cached("SELECT COUNT(*) FROM term_occurrences")?
        .query_row(params![], |row| row.get::<_, i64>(0))?;

    if text_occurences_count == 0 {
        let tx = conn.transaction()?;
        tfidf::fill_word_occurence_table(&tx)?;
        tx.commit()?;
    }

    // Check if subjects.parent_id exists
    let parent_id_exists = conn
        .prepare_cached(
            "SELECT * FROM pragma_table_info('subjects')
            WHERE name = 'parent_id' LIMIT 1",
        )?
        .query_row(params![], |_| Ok(()))
        .is_ok();

    if !parent_id_exists {
        conn.execute_batch("ALTER TABLE subjects ADD COLUMN parent_id BLOB;")?;
    }

    let done_at_exists = conn
        .prepare_cached(
            "SELECT * FROM pragma_table_info('notes')
            WHERE name = 'done_at' LIMIT 1",
        )?
        .query_row(params![], |_| Ok(()))
        .is_ok();

    if !done_at_exists {
        conn.execute_batch("
            ALTER TABLE notes ADD COLUMN done_at INTEGER;
            UPDATE notes SET done_at = modified_at WHERE task_state = 2;
        ")?;
    }

    Ok(())
}
