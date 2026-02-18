use dioxus::prelude::*;
use crate::state::TabState;

#[component]
pub fn TabBar(
    tabs: Vec<TabState>,
    active_tab: usize,
    on_select: EventHandler<usize>,
    on_close: EventHandler<usize>,
    on_new: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "tab-bar",
            for (idx, tab) in tabs.iter().enumerate() {
                div {
                    class: if idx == active_tab { "tab active" } else { "tab" },
                    onclick: {
                        let idx = idx;
                        move |_| on_select.call(idx)
                    },
                    span { class: "tab-label", "{tab.label}" }
                    button {
                        class: "tab-close",
                        onclick: {
                            let idx = idx;
                            move |evt: Event<MouseData>| {
                                evt.stop_propagation();
                                on_close.call(idx);
                            }
                        },
                        "×"
                    }
                }
            }
            button {
                class: "tab-new",
                onclick: move |_| on_new.call(()),
                "＋"
            }
        }
    }
}
