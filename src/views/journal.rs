use dioxus::prelude::*;
use emergence::data::query::use_store;

use crate::{
    data::{
        query::use_subject_query,
        subjects::{Subject, SubjectId},
    },
    views::{list_notes::ListNotes, search_view::{Search, SearchText, SearchOpen}, select_subject::SelectSubject},
};

pub struct SelectedSubject(pub Option<SubjectId>);

impl std::ops::Deref for SelectedSubject {
    type Target = Option<SubjectId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn Journal(cx: Scope) -> Element {
    use_shared_state_provider(cx, || SelectedSubject(None));
    use_shared_state_provider(cx, || SearchText(String::new()));
    use_shared_state_provider(cx, || SearchOpen(false));

    let subjects = use_subject_query(cx).subjects();
    let my_subject = use_shared_state::<SelectedSubject>(cx).unwrap();

    let subject_name = my_subject
        .read()
        .and_then(|id| subjects.get(&id))
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "Journal".to_string());

    let show_subject_select = use_state(cx, || false);
    let tasks_only = use_state(cx, || false);
    let show_search = use_shared_state::<SearchOpen>(cx).unwrap();

    let store = use_store(cx);
    let note_count = my_subject
        .read()
        .map_or(0, |s| store.read().subject_note_count(s).unwrap());

    let delete_subject = move || {
        let mut subject = my_subject.write();
        if let Some(id) = subject.0 {
            store.read().delete_subject(id).unwrap();
        }
        subject.0 = None;
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
                        if note_count == 0 && my_subject.read().0 != None {
                            rsx! {
                                button {
                                    class: "select-button",
                                    onclick: move |_| delete_subject(),
                                    "Delete Subject"
                                }
                            }
                        }
                        button {
                            class: if show_search.read().0 {
                                "select-button selected"
                            } else {
                                "select-button"
                            },
                            onclick: |_| {
                                let mut show_search = show_search.write();
                                show_search.0 = !show_search.0;
                            },
                            "Search"
                        }
                        button {
                            class: if *tasks_only.get() {
                                "select-button selected"
                            } else {
                                "select-button"
                            },
                            onclick: |_| tasks_only.set(!*tasks_only.get()),
                            "Tasks Only"
                        }
                        if my_subject.read().0 != None {
                            rsx! {
                                button {
                                    class: "select-button",
                                    onclick: |_| my_subject.write().0 = None,
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
                                on_select: |subject: Subject| {
                                    my_subject.write().0 = Some(subject.id);
                                    show_subject_select.set(false);
                                },
                                on_cancel: |_| show_subject_select.set(false),
                                ignore_subjects: vec![],
                            }
                        }
                    }
                }
            },
            if show_search.read().0 {
                rsx! {
                    Search { }
                }
            } else {
                rsx! {
                    div {
                        class: "notes",
                        ListNotes {
                            tasks_only: *tasks_only.get(),
                        }
                    }
                }
            }
        }
    }
}
