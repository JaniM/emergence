
use dioxus::prelude::*;

use crate::{data::{query::use_subject_query, subjects::{SubjectId, Subject}}, views::{list_notes::ListNotes, select_subject::SelectSubject}};

pub fn Journal(cx: Scope) -> Element {
    let subjects = use_subject_query(cx).subjects();
    let my_subject = use_state(cx, || None::<SubjectId>);

    let subject_name = my_subject
        .and_then(|id| subjects.get(&id))
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "Journal".to_string());

    let show_subject_select = use_state(cx, || false);

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
                        if *my_subject.get() != None {
                            rsx! {
                                button {
                                    class: "select-button",
                                    onclick: |_| my_subject.set(None),
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
                                    my_subject.set(Some(subject.id));
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
                    subject: *my_subject.get(),
                    on_select_subject: |subject| {
                        my_subject.set(Some(subject));
                    },
                }
            }
        }
    }
}