use rusqlite::functions::{Aggregate, Context, FunctionFlags};
use rusqlite::{Connection, Result, ToSql};
use smallvec::SmallVec;

/// Wrapper around SmallVec to implement ToSql.
/// 32 bytes is enough for 2 UUIDs, which I assume is enough for most notes.
#[derive(Default)]
struct Blob(SmallVec<[u8; 32]>);

impl ToSql for Blob {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

pub fn add_functions(conn: &Connection) -> Result<()> {
    add_concat_blobs(conn)?;
    add_case_insensitive_includes(conn)?;
    Ok(())
}

fn add_concat_blobs(conn: &Connection) -> Result<()> {
    struct ConcatBlobs;

    impl Aggregate<Blob, Blob> for ConcatBlobs {
        fn init(&self, _ctx: &mut Context<'_>) -> rusqlite::Result<Blob> {
            Ok(Blob::default())
        }

        fn step(&self, ctx: &mut Context<'_>, result: &mut Blob) -> rusqlite::Result<()> {
            let blob = ctx.get_raw(0).as_blob_or_null()?;
            if let Some(blob) = blob {
                result.0.extend_from_slice(blob);
            }
            Ok(())
        }

        fn finalize(&self, _: &mut Context<'_>, result: Option<Blob>) -> Result<Blob> {
            Ok(result.unwrap_or_default())
        }
    }

    conn.create_aggregate_function(
        "concat_blobs",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        ConcatBlobs,
    )
}

pub fn add_case_insensitive_includes(conn: &Connection) -> Result<()> {
    fn cmp_insensitive(a: &str, b: &str) -> bool {
        a.to_lowercase().contains(&b.to_lowercase())
    }

    conn.create_scalar_function(
        "case_insensitive_includes",
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        move |ctx| {
            let haystack = ctx.get_raw(0).as_str()?;
            let needle = ctx.get_raw(1).as_str()?;
            Ok(cmp_insensitive(haystack, needle))
        },
    )
}
