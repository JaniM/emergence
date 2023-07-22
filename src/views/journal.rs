use dioxus::prelude::*;
use emergence::data::query::use_store;

use crate::{
    data::{query::use_subject_query, subjects::Subject},
    views::{list_notes::ListNotes, search_view::Search, select_subject::SelectSubject, ViewState},
};

pub fn Journal(cx: Scope) -> Element {
    let view_state = use_shared_state::<ViewState>(cx).unwrap();

    let subjects = use_subject_query(cx).subjects();

    let ViewState {
        tasks_only,
        show_search,
        selected_subject,
        ..
    } = &*view_state.read();

    let subject_name = selected_subject
        .and_then(|id| subjects.get(&id))
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "Journal".to_string());

    let show_subject_select = use_state(cx, || false);

    let store = use_store(cx);
    let note_count = selected_subject.map_or(0, |s| store.read().subject_note_count(s).unwrap());

    let delete_subject = move || {
        let mut state = view_state.write();
        let subject = &mut state.selected_subject;
        if let Some(id) = *subject {
            store.read().delete_subject(id).unwrap();
        }
        *subject = None;
    };

    let jump_to_subject = move |subject: Option<Subject>| {
        let mut state = view_state.write();
        if let Some(subject) = subject.as_ref() {
            state.go_to_subject(subject.id);
        } else {
            state.go_to_journal();
        }
        show_subject_select.set(false);
    };

    render! {
        div {
            class: "journal",
            div {
                class: "header",
                div { },
                div {
                    class: "title",
                    subject_name
                },
                div {
                    class: "select-column",
                    div {
                        class: "row",
                        if note_count == 0 && *selected_subject != None {
                            rsx! {
                                button {
                                    class: "select-button",
                                    onclick: move |_| delete_subject(),
                                    "Delete Subject"
                                }
                            }
                        }
                        button {
                            class: if *show_search {
                                "select-button selected"
                            } else {
                                "select-button"
                            },
                            onclick: |_| {
                                view_state.write().toggle_search();
                            },
                            "Search"
                        }
                        button {
                            class: if *tasks_only {
                                "select-button selected"
                            } else {
                                "select-button"
                            },
                            onclick: |_| {
                                view_state.write().toggle_tasks_only();
                            },
                            "Tasks Only"
                        }
                        if *selected_subject != None {
                            rsx! {
                                button {
                                    class: "select-button",
                                    onclick: move |_| jump_to_subject(None),
                                    "Journal"
                                }
                            }
                        }
                        button {
                            class: "select-button",
                            onclick: |_| show_subject_select.set(!*show_subject_select.get()),
                            "Select Subject"
                        }
                    }
                    if *show_subject_select.get() {
                        rsx! {
                            SelectSubject {
                                on_select: move |s| jump_to_subject(Some(s)),
                                on_cancel: |_| show_subject_select.set(false),
                                ignore_subjects: vec![],
                            }
                        }
                    }
                }
            },
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
