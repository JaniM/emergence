use crate::{
    data::{
        notes::Note,
        query::{use_note_query, use_store, use_subject_query},
        subjects::{Subject, SubjectId},
    },
    ShowInput,
};
use dioxus::prelude::*;
use std::{collections::BTreeMap, rc::Rc};

use super::select_subject::SelectSubject;
use dioxus::html::input_data::keyboard_types::{Key, Modifiers};

#[derive(Props)]
pub struct ListNotesProps<'a> {
    #[props(!optional)]
    pub subject: Option<SubjectId>,
    pub on_select_subject: EventHandler<'a, SubjectId>,
}

pub fn ListNotes<'a>(cx: Scope<'a, ListNotesProps<'a>>) -> Element<'a> {
    let query = use_note_query(cx, cx.props.subject).notes();
    let show_input = use_shared_state::<ShowInput>(cx).unwrap();

    let mut groups = BTreeMap::new();
    for node in query.iter() {
        let date = node.created_at.naive_local().date();
        groups.entry(date).or_insert_with(Vec::new).push(node);
    }

    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.reverse();

    let mut groups = groups
        .into_iter()
        .map(|(date, nodes)| {
            (
                date,
                nodes
                    .into_iter()
                    .map(|note| {
                        rsx! { ViewNote {
                            key: "{note.id.0}",
                            note: note.clone(),
                            on_select_subject: |subject: Subject| {
                                cx.props.on_select_subject.call(subject.id);
                            },
                        } }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    let today = chrono::Local::now().naive_local().date();
    if groups.is_empty() || groups[0].0 != today {
        groups.insert(0, (today, vec![]));
    }

    let add_note = if show_input.read().0 {
        rsx! {
            NoteInput {
                key: "input",
                subject: cx.props.subject,
                on_create_note: |_| show_input.write().0 = false,
                on_cancel: |_| show_input.write().0 = false
            }
        }
    } else {
        rsx! {
            button {
                key: "add-note-button",
                class: "add-note",
                onclick: |_| {
                    show_input.write().0 = true;
                },
                "Add note"
            }
        }
    };

    groups[0].1.insert(0, add_note);

    render! {
        div {
            class: "note-grid-wrapper",
            div {
                class: "note-grid",
                groups.into_iter().map(|(date, nodes)| {
                    rsx! {
                        div {
                            key: "{date.format(\"%Y-%m-%d\")}",
                            class: "group-wrapper",
                            div {
                                class: "date",
                                r#"{date.format("%Y-%m-%d")}"#
                            },
                            div {
                                class: "group",
                                nodes.into_iter()
                            }
                        }
                    }
                })
            }
        }
    }
}

#[derive(Props)]
struct ViewNoteProps<'a> {
    note: Note,
    on_select_subject: EventHandler<'a, Subject>,
}

fn ViewNote<'a>(cx: Scope<'a, ViewNoteProps<'a>>) -> Element<'a> {
    let note = &cx.props.note;
    let time_text = note
        .created_at
        .naive_local()
        .format("%Y-%m-%d %H:%M")
        .to_string();
    cx.render(rsx! {
        div {
            class: "note",
            SubjectCards {
                sids: note.subjects.clone(),
                on_click_subject: |subject: Subject| {
                    cx.props.on_select_subject.call(subject);
                },
            },
            div {
                class: "note-content",
                title: "{time_text}",
                "{cx.props.note.text}",
            },
        }
    })
}

#[derive(Props)]
struct SubjectCardsProps<'a> {
    sids: Vec<SubjectId>,
    on_add_subject: Option<EventHandler<'a, ()>>,
    on_click_subject: Option<EventHandler<'a, Subject>>,
}

fn SubjectCards<'a>(cx: Scope<'a, SubjectCardsProps<'a>>) -> Element<'a> {
    let subjects = use_subject_query(cx).subjects();
    let mut cards = cx
        .props
        .sids
        .iter()
        .map(|sid| {
            let s = subjects.get(sid).unwrap().clone();
            let on_click_subject = &cx.props.on_click_subject;
            rsx! {
                div {
                    key: "{s.id.0}",
                    class: "subject-card",
                    onclick: move |_| {
                        if let Some(on_click_subject) = on_click_subject {
                            on_click_subject.call(s.clone());
                        }
                    },
                    "{s.name}"
                }
            }
        })
        .collect::<Vec<_>>();
    if let Some(on_add_subject) = &cx.props.on_add_subject {
        cards.push(rsx! {
            div {
                key: "add-subject",
                class: "subject-card",
                onclick: |_| on_add_subject.call(()),
                "+"
            }
        });
    }
    cx.render(rsx! {
        div {
            class: "note-subjects",
            cards.into_iter()
        }
    })
}

#[derive(Props)]
struct NoteInputProps<'a> {
    #[props(!optional)]
    subject: Option<SubjectId>,
    on_create_note: EventHandler<'a, String>,
    on_cancel: EventHandler<'a, ()>,
}

fn NoteInput<'a>(cx: Scope<'a, NoteInputProps<'a>>) -> Element<'a> {
    #[derive(PartialEq, Eq, Clone, Copy)]
    enum ShowSubjects {
        No,
        YesKeyboard,
        YesMouse,
    }

    let store = use_store(cx);
    let text = use_state(cx, String::new);
    let subjects = use_ref(cx, || cx.props.subject.into_iter().collect::<Vec<_>>());
    let show_subjects = use_state(cx, || ShowSubjects::No);
    let textarea = use_state(cx, || None::<Rc<MountedData>>);

    let rows = text.matches("\n").count() as i64 + 1;

    let onkeypress = |e: KeyboardEvent| match e.key() {
        Key::Enter if e.modifiers().contains(Modifiers::CONTROL) => {
            if !text.is_empty() {
                store
                    .read()
                    .add_note(text.get().clone(), subjects.read().clone())
                    .unwrap();
            }
            cx.props.on_create_note.call(text.get().clone());
            text.set(String::new());
        }
        Key::Escape => {
            cx.props.on_cancel.call(());
        }
        Key::Character(c) if c == "@" && *show_subjects.get() == ShowSubjects::No => {
            show_subjects.set(ShowSubjects::YesKeyboard);
        }
        _ => {}
    };

    let on_select_subject = |subject: Subject| {
        subjects.write().push(subject.id);
        show_subjects.set(ShowSubjects::No);
        textarea.get().as_ref().unwrap().set_focus(true);
        if *show_subjects.get() == ShowSubjects::YesKeyboard
            && text.get().chars().last() == Some('@')
        {
            // remove the @
            text.set(text.get()[0..text.get().len() - 1].to_string());
        }
    };

    cx.render(rsx! {
        div {
            class: "note",
            SubjectCards {
                sids: subjects.read().clone(),
                on_add_subject: |_| show_subjects.set(ShowSubjects::YesMouse),
                on_click_subject: |subject: Subject| {
                    subjects.write().retain(|s| *s != subject.id);
                    textarea.get().as_ref().unwrap().set_focus(true);
                },
            },
            div {
                class: "note-content",
                textarea {
                    rows: rows,
                    value: "{text}",
                    onmounted: |e| {
                        textarea.set(Some(e.inner().clone()));
                        e.inner().set_focus(true);
                    },
                    oninput: |e| text.set(e.value.clone()),
                    onkeypress: onkeypress,
                }
            },
            if *show_subjects.get() != ShowSubjects::No {
                rsx! {
                    SelectSubject {
                        on_select: on_select_subject,
                        on_cancel: |_| {
                            show_subjects.set(ShowSubjects::No);
                            textarea.get().as_ref().unwrap().set_focus(true);
                        },
                        ignore_subjects: subjects.read().clone(),
                    }
                }
            }
        }
    })
}
