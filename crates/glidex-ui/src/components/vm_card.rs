use leptos::prelude::*;

use crate::components::{VmAction, VmActions};
use crate::types::VmResponse;

#[component]
pub fn VmCard(
    vm: VmResponse,
    #[prop(into)] on_action: Callback<(String, VmAction)>,
) -> impl IntoView {
    let state_class = format!("px-2 py-1 text-xs font-medium text-white rounded-full {}", vm.state.css_class());
    let state_text = vm.state.display();
    let vm_name = vm.name.clone();
    let vm_id_display = vm.id.clone();
    let vm_id = vm.id.clone();
    let vm_id_link = vm.id.clone();
    let vm_state = vm.state.clone();
    let mem_display = format!("{} MiB", vm.mem_size_mib);
    let vcpu_display = vm.vcpu_count;
    let link_href = format!("/vms/{}", vm_id_link);

    view! {
        <div class="bg-white rounded-xl shadow-md p-6 border border-gray-100 hover:shadow-lg transition-shadow duration-200">
            <div class="flex items-start justify-between">
                <div class="flex-1 min-w-0">
                    <div class="flex items-center space-x-3">
                        <h3 class="text-lg font-semibold text-gray-900 truncate">
                            {vm_name}
                        </h3>
                        <span class=state_class>
                            {state_text}
                        </span>
                    </div>
                    <p class="mt-1 text-sm text-gray-500 font-mono truncate">
                        {vm_id_display}
                    </p>
                </div>
            </div>

            <div class="mt-4 grid grid-cols-2 gap-4 text-sm">
                <div>
                    <span class="text-gray-500">"vCPUs:"</span>
                    <span class="ml-2 font-medium text-gray-900">{vcpu_display}</span>
                </div>
                <div>
                    <span class="text-gray-500">"Memory:"</span>
                    <span class="ml-2 font-medium text-gray-900">{mem_display}</span>
                </div>
            </div>

            <div class="mt-4 pt-4 border-t border-gray-100">
                <VmActions
                    vm_id=vm_id
                    state=vm_state
                    on_action=on_action
                />
            </div>

            <a
                href=link_href
                class="mt-3 inline-block text-sm text-sky-600 hover:text-sky-700"
            >
                "View Details"
            </a>
        </div>
    }
}
