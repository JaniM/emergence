
use dioxus::prelude::*;

use crate::{
    data::{
        query::use_subject_query,
        subjects::{Subject, SubjectId},
    },
    views::{list_notes::ListNotes, select_subject::SelectSubject},
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

    let subjects = use_subject_query(cx).subjects();
    let my_subject = use_shared_state::<SelectedSubject>(cx).unwrap();

    let subject_name = my_subject.read()
        .and_then(|id| subjects.get(&id))
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "Journal".to_string());

    let show_subject_select = use_state(cx, || false);
    let tasks_only = use_state(cx, || false);

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
            div {
                class: "notes",
                ListNotes {
                    tasks_only: *tasks_only.get(),
                }
            }
        }
    }
}
