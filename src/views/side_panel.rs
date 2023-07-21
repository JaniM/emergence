use dioxus::prelude::*;
use emergence::data::{
    query::{use_store, use_store_event_query, use_subject_query},
    subjects::SubjectId,
};

use crate::views::view_note::ViewNote;

#[derive(Default, Clone)]
pub enum SidePanelState {
    #[default]
    Nothing,
    SubjectDetails(SubjectId),
    ListSimilar {
        text: String,
        previous: Box<SidePanelState>,
    },
}

impl SidePanelState {
    pub fn list_similar(&mut self, text: String) {
        let old = std::mem::replace(self, Self::Nothing);
        *self = match old {
            Self::ListSimilar { text: _, previous } => Self::ListSimilar {
                text,
                previous: previous.clone(),
            },
            _ => Self::ListSimilar {
                text,
                previous: Box::new(old),
            },
        }
    }

    pub fn back(&mut self) {
        let old = std::mem::replace(self, Self::Nothing);
        *self = match old {
            Self::ListSimilar { text: _, previous } => *previous,
            _ => old,
        }
    }
}

pub fn SidePanel(cx: Scope) -> Element {
    let state = use_shared_state::<SidePanelState>(cx).unwrap();

    let state = state.read();
    let content = match &*state {
        SidePanelState::Nothing => rsx! {
            div {
            }
        },
        SidePanelState::SubjectDetails(subject) => rsx! {
            SubjectDetails {
                subject_id: subject.clone(),
            }
        },
        SidePanelState::ListSimilar { text, .. } => rsx! {
            FindSimilar {
                text: text.clone(),
            }
        },
    };

    cx.render(rsx! {
        div {
            class: "side-panel",
            content
        }
    })
}

#[inline_props]
fn SubjectDetails(cx: Scope, subject_id: SubjectId) -> Element {
    let subjects = use_subject_query(cx).subjects();
    let subject = subjects.get(&subject_id).unwrap().clone();
    cx.render(rsx! {
        div {
            class: "side-panel-header",
            "Subject Details: {subject.name}"
        }
    })
}

#[inline_props]
fn FindSimilar(cx: Scope, text: String) -> Element {
    let search = use_store(cx).read().search.clone();

    let counter = use_store_event_query(cx).count();

    let similar = use_future(cx, (text, &counter), |(text, _)| async move {
        search.find_similar(text.clone()).await
    });

    let notes = similar.value()?;

    let elems = notes
        .iter()
        .map(|note| {
            rsx! {
                div {
                    style: "max-height: 100px; height: fit-content; overflow: hidden;",
                    ViewNote {
                        key: "{note.id.0}",
                        note: note.clone(),
                        on_select_subject: |_| {},
                        hide_subject: None,
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    cx.render(rsx! {
        div {
            class: "side-panel-header",
            "Similar Notes"
        }
        div {
            class: "similar-notes",
            elems.into_iter()
        }
    })
}
