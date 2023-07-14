use dioxus::prelude::*;

#[inline_props]
pub fn Markdown(cx: Scope, source: String) -> Element {
    let html = markdown::to_html(&source);
    render! {
        div {
            class: "markdown allow-select",
            dangerous_inner_html: "{html}"
        }
    }
}
