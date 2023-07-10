#![allow(non_snake_case)]

mod views;

use dioxus_desktop::{use_eval, use_window};
pub use emergence::data;

use data::Store;
use dioxus::{
    html::input_data::keyboard_types::{Key, Modifiers},
    prelude::*,
};
use tracing::{metadata::LevelFilter, trace};

use crate::views::journal::Journal;

fn main() {
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(LevelFilter::TRACE)
            .finish(),
    )
    .unwrap();

    trace!("Starting app");

    // launch the dioxus app in a webview
    dioxus_desktop::launch_cfg(
        App,
        dioxus_desktop::Config::new().with_window(
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

fn App(cx: Scope) -> Element {
    // TODO: Use a context provider for this
    use_shared_state_provider(cx, || {
        Store::new(data::ConnectionType::File("data.db".into()))
    });
    use_shared_state_provider(cx, || ShowInput(false));
    let show_input = use_shared_state::<ShowInput>(cx).unwrap();
    let window = use_window(cx);
    let zoom_level = use_state(cx, || 100);

    // Workaround for not being able to attach event listeners to the document.
    let js = "
        if (!window.eventsRegistered) {
            document.addEventListener('focusout', (e) => {
                if (e.relatedTarget === null) {
                    document.querySelector('.magic-capture').focus();
                }
            });

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
                tabindex: 1000,
                autofocus: true,
                onkeydown: onkeydown,
            }
            Journal { },
        }
    }
}
