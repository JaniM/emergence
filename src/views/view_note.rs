use dioxus::prelude::*;
use emergence::data::{
    notes::Note,
    query::use_subject_query,
    subjects::{Subject, SubjectId},
};

#[derive(Props)]
pub struct ViewNoteProps<'a> {
    note: Note,
    on_select_subject: EventHandler<'a, Subject>,
}

pub fn ViewNote<'a>(cx: Scope<'a, ViewNoteProps<'a>>) -> Element<'a> {
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
pub struct SubjectCardsProps<'a> {
    sids: Vec<SubjectId>,
    on_add_subject: Option<EventHandler<'a, ()>>,
    on_click_subject: Option<EventHandler<'a, Subject>>,
}

pub fn SubjectCards<'a>(cx: Scope<'a, SubjectCardsProps<'a>>) -> Element<'a> {
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
