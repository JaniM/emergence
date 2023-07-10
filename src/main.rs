#![allow(non_snake_case)]

mod views;

pub use emergence::data;

use std::rc::Rc;

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
    use_shared_state_provider(cx, || {
        Store::new(data::ConnectionType::File("data.db".into()))
    });
    use_shared_state_provider(cx, || ShowInput(false));
    let show_input = use_shared_state::<ShowInput>(cx).unwrap();
    let magic_capture_ref = use_ref(cx, || None::<Rc<MountedData>>);

    if let Some(magic_capture) = &*magic_capture_ref.read() {
        if !show_input.read().0 {
            magic_capture.set_focus(true);
        }
    }

    let onkeydown = move |e: KeyboardEvent| match e.key() {
        Key::Character(c) if c == "n" && e.modifiers().contains(Modifiers::CONTROL) => {
            show_input.write().0 = true;
        }
        _ => {}
    };

    render! {
        div {
            class: "app",
            onkeydown: onkeydown,
            style { include_str!("style.css") },
            div {
                class: "magic-capture",
                tabindex: 0,
                onmounted: |e| {
                    magic_capture_ref.set(Some(e.inner().clone()));
                },
            }
            Journal { },
        }
    }
}
