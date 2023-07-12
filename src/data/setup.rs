use rusqlite::{params, Connection, Result};

pub fn setup_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS subjects (
            id BLOB PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        ) STRICT;

        CREATE TABLE IF NOT EXISTS notes (
            id BLOB PRIMARY KEY,
            text TEXT NOT NULL,
            -- 0 = not a task, 1 = incomplete, 2 = complete
            task_state INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            modified_at INTEGER NOT NULL
        ) STRICT;

        CREATE INDEX IF NOT EXISTS notes_created_at_index ON notes (created_at);

        CREATE TABLE IF NOT EXISTS notes_subjects (
            note_id BLOB NOT NULL,
            subject_id BLOB NOT NULL,
            PRIMARY KEY (note_id, subject_id),
            FOREIGN KEY (note_id) REFERENCES notes(id),
            FOREIGN KEY (subject_id) REFERENCES subjects(id)
        ) STRICT;

        CREATE TABLE IF NOT EXISTS notes_search (
            note_id BLOB NOT NULL,
            subject_id BLOB NOT NULL,
            created_at INTEGER NOT NULL,
            PRIMARY KEY (note_id, subject_id),
            FOREIGN KEY (note_id) REFERENCES notes(id),
            FOREIGN KEY (subject_id) REFERENCES subjects(id)
        ) STRICT;

        CREATE INDEX IF NOT EXISTS notes_search_index ON notes_search (subject_id, created_at, note_id);

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

    // Add notes.task_state column if it doesn't exist
    let task_state_exists = conn
        .prepare_cached(
            "SELECT 1 FROM pragma_table_info('notes')
            WHERE name = 'task_state'",
        )?
        .query_row(params![], |_| Ok(true))
        .unwrap_or(false);

    if !task_state_exists {
        conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN task_state INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

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
