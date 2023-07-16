use rusqlite::{params, Connection, Result};

pub fn setup_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS subjects (
            id BLOB PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        ) WITHOUT ROWID, STRICT;

        CREATE TABLE IF NOT EXISTS notes (
            id BLOB PRIMARY KEY,
            text TEXT NOT NULL,
            -- 0 = not a task, 1 = incomplete, 2 = complete
            task_state INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            modified_at INTEGER NOT NULL
        ) STRICT;

        CREATE INDEX IF NOT EXISTS notes_created_at_index ON notes (created_at);
        CREATE INDEX IF NOT EXISTS notes_tasks_index ON notes(task_state ASC, created_at DESC);


        CREATE TABLE IF NOT EXISTS notes_subjects (
            note_id BLOB NOT NULL,
            subject_id BLOB NOT NULL,
            PRIMARY KEY (note_id, subject_id),
            FOREIGN KEY (note_id) REFERENCES notes(id),
            FOREIGN KEY (subject_id) REFERENCES subjects(id)
        ) WITHOUT ROWID, STRICT;

        CREATE TABLE IF NOT EXISTS notes_search (
            note_id BLOB NOT NULL,
            subject_id BLOB NOT NULL,
            created_at INTEGER NOT NULL,
            task_state INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (note_id) REFERENCES notes(id),
            FOREIGN KEY (subject_id) REFERENCES subjects(id)
        ) STRICT;

        CREATE INDEX IF NOT EXISTS notes_search_index ON notes_search (subject_id, created_at);
        CREATE INDEX IF NOT EXISTS notes_search_tasks_index ON notes_search (subject_id, task_state ASC, created_at DESC);


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
            WHERE note_id = OLD.note_id AND subject_id = OLD.subject_id;
        END;

        CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
            text,
            content=notes,
            tokenize="trigram"
        );

        CREATE TRIGGER IF NOT EXISTS notes_fts_insert AFTER INSERT ON notes BEGIN
            INSERT INTO notes_fts (rowid, text)
            VALUES (NEW.rowid, NEW.text);
        END;

        CREATE TRIGGER IF NOT EXISTS notes_fts_update AFTER UPDATE ON notes BEGIN
            INSERT INTO notes_fts (notes_fts, rowid, text)
            VALUES('delete', OLD.rowid, OLD.text);
            INSERT INTO notes_fts (rowid, text)
            VALUES (NEW.rowid, NEW.text);
        END;

        CREATE TRIGGER IF NOT EXISTS notes_fts_delete AFTER DELETE ON notes BEGIN
            INSERT INTO notes_fts (notes_fts, rowid)
            VALUES('delete', OLD.rowid);
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

    let notes_fts_count = conn
        .prepare_cached("SELECT COUNT(*) FROM notes_fts_idx")?
        .query_row(params![], |row| row.get::<_, i64>(0))?;

    if notes_fts_count == 0 {
        conn.execute_batch(
            r#"
            INSERT INTO notes_fts (rowid, text)
            SELECT rowid, text FROM notes;
        "#,
        )?;
    }

    Ok(())
}
