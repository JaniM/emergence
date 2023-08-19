use dioxus::prelude::*;

use crate::views::list_notes::ListSearchResult;

use super::use_view_state;

pub fn Search(cx: Scope) -> Element {
    let view_state = use_view_state(cx);
    let text = view_state.read().search_text.clone();

    let has_too_short_word = text.split_whitespace().any(|word| word.len() < 3);

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
                    oninput: move |e| {
                        view_state.write().set_search_text(e.value.clone());
                    },
                },
            }
            div {
                style: "overflow-y: scroll; max-height: 100%;",
                if has_too_short_word {
                    rsx! {
                        div {
                            class: "group-wrapper",
                            div {
                                class: "date-wrapper",
                            }
                            div {
                                class: "group",
                                div {
                                    class: "note",
                                    "Search terms must be at least 3 characters long"
                                }
                            }
                        }
                    }
                }
                ListSearchResult {
                    search_text: text.clone(),
                }
            }
        }
    })
}
