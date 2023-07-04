#![allow(non_snake_case)]

mod data;
mod views;

use data::Store;
use dioxus::prelude::*;
use views::list_notes::ListNotes;

fn main() {
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

fn App(cx: Scope) -> Element {
    use_shared_state_provider(cx, Store::new);
    let store = use_store(cx);

    let show_input = use_state(cx, || false);

    dioxus_desktop::use_global_shortcut(cx, "ctrl+n", {
        to_owned![show_input];
        move || {
            show_input.set(true);
        }
    });

    render! {
        div {
            style { include_str!("style.css") },
            ListNotes {
                notes: store.read().notes.get_notes(),
                create_note: *show_input.get(),
                on_create_note: |text: String| {
                    if !text.is_empty() {
                        store.write().notes.add(text);
                    }
                    show_input.set(false);
                },
            },
        }
    }
}

fn use_store(cx: Scope) -> &UseSharedState<data::Store> {
    use_shared_state(cx).expect("Store context not set")
}
