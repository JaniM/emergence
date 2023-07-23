use dioxus::prelude::*;
use sir::css;

use crate::views::{list_notes::ListNotes, search_view::Search, ViewState};

pub fn Journal(cx: Scope) -> Element {
    let view_state = use_shared_state::<ViewState>(cx).unwrap();

    let ViewState { show_search, .. } = &*view_state.read();

    let style = css!(
        "
        display: grid;
        grid-template-columns: 1fr;
        grid-template-rows: auto 1fr;
        max-width: 800px;
        position: relative;

        .notes {
            max-height: 100%;
            display: grid;
            overflow: hidden;
        }
    "
    );

    render! {
        div {
            class: "{style}",
            Tabs { },
            if *show_search {
                rsx! {
                    Search { }
                }
            } else {
                rsx! {
                    div {
                        class: "notes",
                        ListNotes { }
                    }
                }
            }
        }
    }
}

fn Tabs(cx: Scope) -> Element {
    let view_state = use_shared_state::<ViewState>(cx).unwrap();

    let ViewState {
        tasks_only,
        show_search,
        ..
    } = &*view_state.read();

    let notes_obly = !*tasks_only && !*show_search;

    let style = css!(
        "
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        grid-template-rows: 1fr;
        height: fit-content;
        gap: 0px;

        .tab {
            display: flex;
            flex-direction: row;
            justify-content: center;
            align-items: center;
            padding: 5px;
            font-size: 1.2em;
            font-weight: bold;
            border-bottom: 2px solid transparent;

            &.selected {
                border-bottom: 2px solid black;
            }
        }
    "
    );

    let tab_class = |selected: bool| {
        if selected {
            "tab selected"
        } else {
            "tab"
        }
    };

    render! {
        div {
            class: "{style}",
            div {
                class: tab_class(notes_obly),
                onclick: |_| {
                    view_state.write().show_notes_only();
                },
                "Notes"
            }
            div {
                class: tab_class(*tasks_only),
                onclick: |_| {
                    view_state.write().show_tasks_only();
                },
                "Tasks"
            }
            div {
                class: tab_class(*show_search),
                onclick: |_| {
                    view_state.write().show_search();
                },
                "Search"
            }
        }
    }
}
