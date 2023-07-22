use dioxus::prelude::*;
use dioxus_desktop::use_eval;
use sir::{css, global_css};

#[derive(Props)]
pub struct ScrollToProps<'a> {
    children: Element<'a>,
}

pub fn ScrollTo<'a>(cx: Scope<'a, ScrollToProps<'a>>) -> Element<'a> {
    let js_eval = use_eval(cx);
    let js = "
        const elem = document.getElementsByClassName('scroll-to')[0];
        elem.scrollIntoView({behavior: 'instant', block: 'center'});
    ";

    global_css!(
        "
        @keyframes scroll-to-flash {
            0% {
                box-shadow: 0;
            }

            25% {
                box-shadow: 0px 0px 5px 5px #c29232;
            }

            100% {
                box-shadow: 0;
            }
        }
    "
    );

    let style = css!(
        "
        animation-duration: 2s;
        animation-name: scroll-to-flash;
    "
    );

    render! {
        div {
            class: "{style} scroll-to",
            onmounted: move |_| {
                js_eval(js.to_string());
            },
            &cx.props.children
        }
    }
}
