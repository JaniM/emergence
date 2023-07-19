use dioxus::prelude::*;
use emergence::data::query::{use_store, use_store_event_query};

use crate::views::view_note::ViewNote;

#[inline_props]
pub fn FindSimilar(cx: Scope, text: String) -> Element {
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
                ViewNote {
                    key: "{note.id.0}",
                    note: note.clone(),
                    on_select_subject: |_| {},
                    hide_subject: None,
                }
            }
        })
        .collect::<Vec<_>>();

    cx.render(rsx! {
        div {
            class: "right-side-popup",
            div {
                class: "right-side-popup-header",
                "Similar Notes"
            }
            elems.into_iter()
        }
    })
}
