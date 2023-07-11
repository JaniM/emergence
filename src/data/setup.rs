use rusqlite::{params, Connection, Result};

pub fn setup_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS subjects (
            id BLOB PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        ) STRICT;
        CREATE TABLE IF NOT EXISTS notes (
            id BLOB PRIMARY KEY,
            text TEXT NOT NULL,
            created_at INTEGER NOT NULL
        ) STRICT;
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