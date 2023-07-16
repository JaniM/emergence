use dioxus::prelude::*;

use crate::views::list_notes::ListSearchResult;

pub struct SearchText(pub String);
pub struct SearchOpen(pub bool);

pub fn Search(cx: Scope) -> Element {
    let search_text = use_shared_state::<SearchText>(cx).unwrap();
    let text = search_text.read().0.clone();

    cx.render(rsx! {
        div {
            class: "search",
            div {
                class: "group-wrapper",
                div {
                    class: "date-wrapper",
                    div {
                        class: "date",
                        "Search"
                    }
                }
                textarea {
                    class: "search-input",
                    value: "{text}",
                    rows: 1,
                    onmounted: |e| {
                        e.inner().set_focus(true);
                    },
                    oninput: |e| {
                        search_text.write().0 = e.value.clone();
                    },
                },
            }
            ListSearchResult {
                search_text: text.clone(),
            }
        }
    })
}