use std::rc::Rc;

use crate::{
    data::subjects::{Subject, SubjectId},
    views::{select_subject::SelectSubject, use_view_state, view_note::SubjectCards},
};
use dioxus::{
    html::input_data::keyboard_types::{Key, Modifiers},
    prelude::*,
};
use emergence::data::{
    layer::use_layer,
    notes::{Note, NoteBuilder, TaskState},
};

#[derive(Props)]
pub struct CreateNoteProps<'a> {
    #[props(!optional)]
    subject: Option<SubjectId>,
    task: bool,
    on_create_note: EventHandler<'a, String>,
    on_cancel: EventHandler<'a, ()>,
}

pub fn CreateNote<'a>(cx: Scope<'a, CreateNoteProps<'a>>) -> Element<'a> {
    let layer = use_layer(cx);

    let on_create_note = move |(text, subjects): (String, Vec<SubjectId>)| {
        if !text.is_empty() {
            let note = NoteBuilder::new(text.clone())
                .subjects(subjects)
                .task_state(if cx.props.task {
                    TaskState::Todo
                } else {
                    TaskState::NotATask
                });
            layer.write().create_note(note);
        }
        cx.props.on_create_note.call(text);
    };

    cx.render(rsx! {
        NoteInput {
            on_create_note: on_create_note,
            on_cancel: |_| cx.props.on_cancel.call(()),
            initial_subjects: cx.props.subject.into_iter().collect(),
        }
    })
}

#[derive(Props)]
pub struct EditNoteProps<'a> {
    note: Note,
    on_done: EventHandler<'a, ()>,
}

pub fn EditNote<'a>(cx: Scope<'a, EditNoteProps<'a>>) -> Element<'a> {
    let layer = use_layer(cx);
    let note_id = cx.props.note.id;

    let on_done = move |_| {
        cx.props.on_done.call(());
    };

    let on_create_note = move |(text, subjects): (String, Vec<SubjectId>)| {
        layer.write().edit_note_with(note_id, |note| {
            note.text = text;
            note.subjects = subjects;
        });
        cx.props.on_done.call(());
    };

    cx.render(rsx! {
        NoteInput {
            on_create_note: on_create_note,
            on_cancel: on_done,
            initial_text: cx.props.note.text.clone(),
            initial_subjects: cx.props.note.subjects.clone(),
        }
    })
}

#[derive(Props)]
struct NoteInputProps<'a> {
    on_create_note: EventHandler<'a, (String, Vec<SubjectId>)>,
    on_cancel: EventHandler<'a, ()>,
    initial_text: Option<String>,
    initial_subjects: Vec<SubjectId>,
}

const TEXTAREA_HACK: &'static str = r#"
const tx = document.getElementsByClassName("note-textarea");
for (let i = 0; i < tx.length; i++) {
    const parent = tx[i].parentElement;
    const parentStyle = window.getComputedStyle(parent, null);
    const parentPadding =
        parseInt(parentStyle.getPropertyValue('padding-bottom'))
        + parseInt(parentStyle.getPropertyValue('padding-top'));
    tx[i].parentElement.setAttribute("style",
        "height:" + (tx[i].scrollHeight + parentPadding) + "px;");
    tx[i].setAttribute("style", "height:0;");
    tx[i].setAttribute("style", "height:" + (tx[i].scrollHeight) + "px;");
    tx[i].parentElement.setAttribute("style", "height: fit-content;");
    tx[i].scrollIntoView({ block: "nearest" });
}
"#;

fn NoteInput<'a>(cx: Scope<'a, NoteInputProps<'a>>) -> Element<'a> {
    #[derive(PartialEq, Eq, Clone, Copy)]
    enum ShowSubjects {
        No,
        YesKeyboard,
        YesMouse,
    }

    // Resize trick
    // Adapted from https://stackoverflow.com/a/25621277
    let js_eval = use_eval(cx);
    let size_textareas = move || {
        js_eval(TEXTAREA_HACK).unwrap();
    };

    // TODO: Combine these states.
    let text = use_state(cx, || cx.props.initial_text.clone().unwrap_or_default());
    let subjects = use_ref(cx, || cx.props.initial_subjects.clone());
    let show_subjects = use_state(cx, || ShowSubjects::No);
    let textarea = use_state(cx, || None::<Rc<MountedData>>);

    let view_state = use_view_state(cx);

    let cleanup = move || {
        view_state.write().side_panel.back();
    };

    let submit = move || {
        let text = text.get().trim().to_string();
        cleanup();
        cx.props
            .on_create_note
            .call((text, subjects.read().clone()));
    };

    let cancel = move || {
        cleanup();
        cx.props.on_cancel.call(());
    };

    let onkeypress = move |e: KeyboardEvent| match e.key() {
        Key::Enter if e.modifiers().contains(Modifiers::CONTROL) => {
            submit();
        }
        Key::Escape => {
            cancel();
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
            class: "note-row",
            SubjectCards {
                sids: subjects.read().clone(),
                on_add_subject: |_| show_subjects.set(ShowSubjects::YesMouse),
                on_click_subject: |subject: Subject| {
                    subjects.write().retain(|s| *s != subject.id);
                    textarea.get().as_ref().unwrap().set_focus(true);
                },
            },
            div {
                class: "note-content note",
                textarea {
                    class: "note-textarea",
                    value: "{text}",
                    rows: 2,
                    onmounted: move |e| {
                        view_state.write().side_panel.list_similar(text.get().clone());
                        textarea.set(Some(e.inner().clone()));
                        e.inner().set_focus(true);
                        size_textareas();
                    },
                    oninput: move |e| {
                        text.set(e.value.clone());
                        size_textareas();
                        view_state.write().side_panel.list_similar(e.value.clone())
                    },
                    onkeypress: onkeypress,
                }
            },
            div {
                class: "note-actions",
                div {
                    class: "row",
                    style: "gap: 0",
                    div {
                        class: "note-action",
                        onclick: move |_| submit(),
                        "Save"
                    }
                    div {
                        class: "note-action",
                        onclick: move |_| cancel(),
                        "Cancel"
                    }
                }
            }
            if *show_subjects.get() != ShowSubjects::No {
                rsx! {
                    SelectSubject {
                        on_select: on_select_subject,
                        on_cancel: |_| {
                            show_subjects.set(ShowSubjects::No);
                            textarea.get().as_ref().unwrap().set_focus(true);
                        },
                        ignore_subjects: subjects.read().clone(),
                        show_above: true
                    }
                }
            }
        }
    })
}
