use leptos::prelude::*;

use crate::types::VmState;

#[derive(Clone, Debug, PartialEq)]
pub enum VmAction {
    Start,
    Stop,
    Pause,
    Delete,
}

#[component]
pub fn VmActions(
    vm_id: String,
    state: VmState,
    #[prop(into)] on_action: Callback<(String, VmAction)>,
    #[prop(default = false)] loading: bool,
) -> impl IntoView {
    let vm_id = StoredValue::new(vm_id);
    let state = StoredValue::new(state);

    let can_start = move || {
        let s = state.get_value();
        s == VmState::Created || s == VmState::Stopped || s == VmState::Paused
    };
    let can_stop = move || {
        let s = state.get_value();
        s == VmState::Running || s == VmState::Paused
    };
    let can_pause = move || state.get_value() == VmState::Running;

    let loading = StoredValue::new(loading);

    view! {
        <div class="flex items-center space-x-2">
            <Show when=can_start>
                <button
                    class="px-3 py-1.5 text-sm font-medium text-white bg-sky-600 hover:bg-sky-700 rounded-lg transition-colors disabled:opacity-50"
                    disabled=move || loading.get_value()
                    on:click=move |_| {
                        on_action.run((vm_id.get_value(), VmAction::Start));
                    }
                >
                    {move || if state.get_value() == VmState::Paused { "Resume" } else { "Start" }}
                </button>
            </Show>

            <Show when=can_pause>
                <button
                    class="px-3 py-1.5 text-sm font-medium text-gray-700 bg-gray-200 hover:bg-gray-300 rounded-lg transition-colors disabled:opacity-50"
                    disabled=move || loading.get_value()
                    on:click=move |_| {
                        on_action.run((vm_id.get_value(), VmAction::Pause));
                    }
                >
                    "Pause"
                </button>
            </Show>

            <Show when=can_stop>
                <button
                    class="px-3 py-1.5 text-sm font-medium text-gray-700 bg-gray-200 hover:bg-gray-300 rounded-lg transition-colors disabled:opacity-50"
                    disabled=move || loading.get_value()
                    on:click=move |_| {
                        on_action.run((vm_id.get_value(), VmAction::Stop));
                    }
                >
                    "Stop"
                </button>
            </Show>

            <button
                class="px-3 py-1.5 text-sm font-medium text-white bg-red-600 hover:bg-red-700 rounded-lg transition-colors disabled:opacity-50"
                disabled=move || loading.get_value()
                on:click=move |_| {
                    on_action.run((vm_id.get_value(), VmAction::Delete));
                }
            >
                "Delete"
            </button>
        </div>
    }
}
