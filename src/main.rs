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
use tracing::{info, metadata::LevelFilter};

use crate::views::journal::Journal;

use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(value_enum, short, long, default_value_t = LogLevel::Info)]
    verbosity: LogLevel,

    /// The database file to use
    #[arg(short, long, value_name = "FILE")]
    db_file: Option<PathBuf>,
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
    let db_file = args.db_file.unwrap_or_else(|| PathBuf::from("data.db"));

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(args.verbosity.to_level_filter())
            .finish(),
    )
    .unwrap();

    info!("Starting app");

    // launch the dioxus app in a webview
    dioxus_desktop::launch_with_props(
        App,
        AppProps { db_file },
        dioxus_desktop::Config::new()
            .with_disable_context_menu(true)
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

fn App(cx: Scope<AppProps>) -> Element {
    // TODO: Use a context provider for this
    use_shared_state_provider(cx, || {
        Store::new(data::ConnectionType::File(cx.props.db_file.clone()))
    });
    use_shared_state_provider(cx, || ShowInput(false));
    let show_input = use_shared_state::<ShowInput>(cx).unwrap();
    let window = use_window(cx);
    let zoom_level = use_state(cx, || 100);

    // Workaround for not being able to attach event listeners to the document.
    let js = "
        if (!window.eventsRegistered) {
            document.addEventListener('keydown', (e) => {
                if (e.key === 'Tab') return;
                if (e.target.className === 'magic-capture') return;
                document
                    .querySelector('.magic-capture')
                    .dispatchEvent(new KeyboardEvent('keydown', e));
            });
            window.eventsRegistered = true;
        }
    ";
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
        div {
            class: "app",
            style { include_str!("style.css") },
            div {
                class: "magic-capture",
                onkeydown: onkeydown,
            }
            Journal { },
        }
    }
}
