#![allow(non_snake_case)]

mod data;
mod views;

use data::Store;
use dioxus::{
    html::input_data::keyboard_types::{Key, Modifiers},
    prelude::*,
};
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

// define a component that renders a div with the text "Hello, world!"
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

    let list = rsx!(ListNotes {
        notes: store.read().notes.get_notes()
    });
    let view = if *show_input.get() {
        rsx!(
            div {
                class: "create-view",
                NoteInput {},
                list,
            }
        )
    } else {
        rsx!(div { list })
    };

    render! {
        div {
            style { include_str!("style.css") },
            view,
        }
    }
}

fn CreateNote(cx: Scope) -> Element {
    cx.render(rsx! {
        div {
            class: "create-note",
            NoteInput {},
        }
    })
}

fn NoteInput(cx: Scope) -> Element {
    let text = use_state(cx, String::new);
    let store = use_store(cx);

    cx.render(rsx! {
        textarea {
            value: "{text}",
            oninput: |e| text.set(e.value.clone()),
            autofocus: true,
            onkeypress: |e| {
                if e.key() == Key::Enter && e.modifiers().contains(Modifiers::CONTROL) {
                    store.write().notes.add(text.to_string());
                    text.set(String::new());
                }
            },
        }
    })
}

fn use_store(cx: Scope) -> &UseSharedState<data::Store> {
    use_shared_state(cx).expect("Store context not set")
}
