use dioxus::prelude::*;

#[inline_props]
pub fn Markdown(cx: Scope, source: String) -> Element {
    let html = markdown::to_html_with_options(source, &markdown::Options::gfm());
    let body = match html {
        Ok(html) => rsx! {
            div {
                class: "markdown allow-select",
                dangerous_inner_html: "{html}"
            }
        },
        Err(e) => {
            tracing::warn!("Markdown error: {}", e);
            rsx! {
                div {
                    class: "markdown allow-select",
                    "{source}"
                }
            }
        }
    };

    cx.render(body)
}
