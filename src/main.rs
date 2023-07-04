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

pub struct ShowInput(pub bool);

fn App(cx: Scope) -> Element {
    use_shared_state_provider(cx, Store::new);
    use_shared_state_provider(cx, || ShowInput(false));
    let show_input = use_shared_state::<ShowInput>(cx).unwrap();

    dioxus_desktop::use_global_shortcut(cx, "ctrl+n", {
        to_owned![show_input];
        move || {
            show_input.write().0 = true;
        }
    });

    render! {
        div {
            style { include_str!("style.css") },
            ListNotes { },
        }
    }
}

fn use_store(cx: &ScopeState) -> &UseSharedState<data::Store> {
    use_shared_state(cx).expect("Store context not set")
}
