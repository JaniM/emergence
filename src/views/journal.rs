use dioxus::prelude::*;
use emergence::data::query::use_store;

use crate::{
    data::{
        query::use_subject_query,
        subjects::{Subject, SubjectId},
    },
    views::{
        list_notes::{ListNotes, ScrollToNote},
        search_view::{Search, SearchOpen, SearchText},
        select_subject::SelectSubject, side_panel::SidePanelState,
    },
};

pub struct SelectedSubject(pub Option<SubjectId>);

impl std::ops::Deref for SelectedSubject {
    type Target = Option<SubjectId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn Journal(cx: Scope) -> Element {
    use_shared_state_provider(cx, || SearchText(String::new()));
    use_shared_state_provider(cx, || SearchOpen(false));
    use_shared_state_provider(cx, || ScrollToNote(None));

    let subjects = use_subject_query(cx).subjects();
    let my_subject = use_shared_state::<SelectedSubject>(cx).unwrap();
    let scroll_to_note = use_shared_state::<ScrollToNote>(cx).unwrap();
    let side_panel = use_shared_state::<SidePanelState>(cx).unwrap();

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

    let jump_to_subject = move |subject: Option<Subject>| {
        my_subject.write().0 = subject.as_ref().map(|s| s.id);
        scroll_to_note.write().0 = None;
        show_subject_select.set(false);
        *side_panel.write() = match subject {
            Some(subject) => SidePanelState::SubjectDetails(subject.id),
            None => SidePanelState::Nothing,
        };
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
