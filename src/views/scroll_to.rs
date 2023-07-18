use dioxus::prelude::*;

#[derive(Props)]
pub struct ScrollToProps<'a> {
    children: Element<'a>,
}

pub fn ScrollTo<'a>(cx: Scope<'a, ScrollToProps<'a>>) -> Element<'a> {
    render! {
        div {
            class: "scroll-to",
            onmounted: move |e| {
                e.inner().scroll_to(ScrollBehavior::Instant);
            },
            div {
                class: "scroll-to-flash",
                &cx.props.children
            }
        }
    }
}
