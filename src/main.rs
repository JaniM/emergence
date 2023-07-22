#![allow(non_snake_case)]

mod views;

use std::path::PathBuf;

use dioxus_desktop::{use_eval, use_window};
pub use emergence::data;

use data::Store;
use dioxus::{
    html::input_data::keyboard_types::{Key, Modifiers},
    prelude::*,
};
use sir::AppStyle;
use tracing::{info, metadata::LevelFilter};

use crate::views::{journal::{Journal, SelectedSubject}, side_panel::{SidePanelState, SidePanel}};

use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(value_enum, short, long, default_value_t = LogLevel::Info)]
    verbosity: LogLevel,

    /// The data folder to use.
    #[arg(short, long, value_name = "FOLDER")]
    data: Option<PathBuf>,

    #[arg(long)]
    reindex: bool,

    /// Construct a sample database.
    ///
    /// This will NOT overwrite an existing database.
    #[arg(long, value_name = "row count")]
    sample: Option<usize>,

    /// Export to JSON file
    #[arg(long, value_name = "FILE", conflicts_with = "import")]
    export: Option<PathBuf>,

    /// Import from JSON file
    #[arg(long, value_name = "FILE", conflicts_with = "export")]
    import: Option<PathBuf>,

    /// Explain database query plans
    #[arg(long)]
    explain: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn to_level_filter(&self) -> LevelFilter {
        match self {
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Error => LevelFilter::ERROR,
        }
    }
}

fn main() {
    let args = Args::parse();
    let data_path = args.data.unwrap_or_else(|| PathBuf::from("data"));

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(args.verbosity.to_level_filter())
            .finish(),
    )
    .unwrap();

    if args.explain {
        info!("Explaining query plans");
        data::explain::explain_all(data::ConnectionType::File(data_path)).unwrap();
        return;
    }

    if args.reindex {
        info!("Reindexing search engine");
        let tantivy_dir = data_path.join("tantivy");
        let data_path = data::ConnectionType::File(data_path);
        let store = Store::new(data_path.clone());
        let conn = store.conn.borrow();

        let _ = std::fs::remove_dir_all(tantivy_dir.clone());
        let index = data::search::construct_tantivy_index(data_path);
        let mut writer = index.writer(50_000_000).unwrap();
        data::search::fill_tantivy_index(&mut writer, &conn);

        info!("Finished reindexing");
        return;
    }

    if let Some(export_file) = args.export {
        info!(
            "Exporting to {}, this may take a long time",
            export_file.display()
        );
        data::export::export(data_path, export_file);
        info!("Finished exporting");
        return;
    }

    if let Some(import_file) = args.import {
        info!(
            "Importing from {}, this may take a long time",
            import_file.display()
        );
        data::export::import(data_path, import_file);
        info!("Finished importing");
        return;
    }

    if let Some(row_count) = args.sample {
        let db_file = data_path.join("data.db");
        if !db_file.exists() {
            info!("Creating sample database, this may take a moment");
            let store = Store::new(data::ConnectionType::File(data_path));
            data::shove_test_data(&mut store.conn.borrow_mut(), row_count).unwrap();
            info!("Finished creating sample database");
        } else {
            info!("Database file already exists, skipping sample database creation");
        }
        return;
    }

    info!("Starting app");

    #[cfg(not(debug_assertions))]
    let disable_context_menu = true;
    #[cfg(debug_assertions)]
    let disable_context_menu = false;

    // launch the dioxus app in a webview
    dioxus_desktop::launch_with_props(
        App,
        AppProps { db_file: data_path },
        dioxus_desktop::Config::new()
            .with_disable_context_menu(disable_context_menu)
            .with_window(
                dioxus_desktop::WindowBuilder::new()
                    .with_title("Emergence Notes")
                    .with_resizable(true)
                    .with_inner_size(dioxus_desktop::wry::application::dpi::LogicalSize::new(
                        1200.0, 800.0,
                    )),
            ),
    );
}

pub struct ShowInput(pub bool);

struct AppProps {
    db_file: PathBuf,
}

fn App<'a>(cx: Scope<'a, AppProps>) -> Element<'a> {
    // TODO: Use a context provider for this
    use_shared_state_provider(cx, || {
        Store::new(data::ConnectionType::File(cx.props.db_file.clone()))
    });
    use_shared_state_provider(cx, || ShowInput(false));
    use_shared_state_provider(cx, || SidePanelState::default());
    use_shared_state_provider(cx, || SelectedSubject(None));

    let show_input = use_shared_state::<ShowInput>(cx).unwrap();
    let window = use_window(cx);
    let zoom_level = use_state(cx, || 100);

    // Workaround for not being able to attach event listeners to the document.
    let js = r#"
        if (!window.eventsRegistered) {
            document.addEventListener('keydown', (e) => {
                // Shift+Tab is recognized ss Unidentified key, so we have to check for code
                if (e.code === 'Tab') return;
                if (e.target.className === 'magic-capture') return;
                document
                    .querySelector('.magic-capture')
                    .dispatchEvent(new KeyboardEvent('keydown', e));
            });
            window.eventsRegistered = true;
        }
    "#;
    use_eval(cx)(js.to_string());

    let onkeydown = move |e: KeyboardEvent| match e.key() {
        Key::Character(c) if e.modifiers().contains(Modifiers::CONTROL) => match c.as_str() {
            "n" => show_input.write().0 = true,
            "+" => {
                let new_zoom = *zoom_level.get() + 10;
                zoom_level.set(new_zoom);
                window.set_zoom_level(new_zoom as f64 / 100.0);
            }
            "-" => {
                let new_zoom = *zoom_level.get() - 10;
                zoom_level.set(new_zoom);
                window.set_zoom_level(new_zoom as f64 / 100.0);
            }
            "0" => {
                zoom_level.set(100);
                window.set_zoom_level(1.0);
            }
            _ => {}
        },
        _ => {}
    };

    render! {
        style { include_str!("style.css") },
        AppStyle { },
        div {
            class: "magic-capture",
            onkeydown: onkeydown,
        }
        div {
            class: "app",
            Journal { },
            SidePanel { },
        }
    }
}
