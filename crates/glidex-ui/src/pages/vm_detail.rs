use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::api;
use crate::components::{Loading, VmAction, VmActions};

#[component]
pub fn VmDetail() -> impl IntoView {
    let params = use_params_map();
    let (error, set_error) = signal(None::<String>);
    let (action_loading, set_action_loading) = signal(false);

    let vm_id = move || params.read().get("id").unwrap_or_default();

    // Resource for fetching VM
    let vm_resource = LocalResource::new(move || {
        let id = vm_id();
        async move {
            if id.is_empty() {
                Err("No VM ID provided".to_string())
            } else {
                api::get_vm(&id).await
            }
        }
    });

    #[allow(unused)]
    let refetch = move || {
        vm_resource.refetch();
    };

    // Handle VM actions
    let handle_action = Callback::new(move |(id, action): (String, VmAction)| {
        set_action_loading.set(true);
        set_error.set(None);

        #[cfg(feature = "hydrate")]
        {
            use leptos::task::spawn_local;
            spawn_local(async move {
                let result = match action {
                    VmAction::Start => api::start_vm(&id).await.map(|_| ()),
                    VmAction::Stop => api::stop_vm(&id).await.map(|_| ()),
                    VmAction::Pause => api::pause_vm(&id).await.map(|_| ()),
                    VmAction::Delete => {
                        match api::delete_vm(&id).await {
                            Ok(_) => {
                                // Redirect to dashboard after deletion
                                if let Some(window) = web_sys::window() {
                                    let location = window.location();
                                    let _ = location.set_href("/");
                                }
                                return;
                            }
                            Err(e) => Err(e),
                        }
                    }
                };

                set_action_loading.set(false);

                if let Err(e) = result {
                    set_error.set(Some(e));
                }

                refetch();
            });
        }

        #[cfg(not(feature = "hydrate"))]
        {
            let _ = (id, action);
            set_action_loading.set(false);
        }
    });

    view! {
        <div>
            <a href="/" class="text-sky-600 hover:text-sky-700 mb-4 inline-flex items-center">
                <svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"></path>
                </svg>
                "Back to Dashboard"
            </a>

            // Error banner
            {move || error.get().map(|e| view! {
                <div class="mb-4 p-4 bg-red-50 border border-red-200 rounded-lg">
                    <div class="flex items-center justify-between">
                        <p class="text-red-700">{e}</p>
                        <button
                            class="text-red-500 hover:text-red-700"
                            on:click=move |_| set_error.set(None)
                        >
                            "Dismiss"
                        </button>
                    </div>
                </div>
            })}

            <Suspense fallback=move || view! { <Loading/> }>
                {move || {
                    vm_resource.get().map(|result| {
                        match result {
                            Ok(vm) => {
                                let state_class = format!("px-3 py-1 text-sm font-medium text-white rounded-full {}", vm.state.css_class());
                                let state_text = vm.state.display();
                                let vm_name = vm.name.clone();
                                let vm_id_display = vm.id.clone();
                                let vm_id = vm.id.clone();
                                let vm_state = vm.state.clone();
                                let console_path = vm.console_socket_path.clone();
                                let log_path_display = vm.log_path.clone();
                                let mem_display = format!("{} MiB", vm.mem_size_mib);
                                let vcpu_display = vm.vcpu_count;

                                view! {
                                    <div class="bg-white rounded-xl shadow-md p-6 border border-gray-100 mt-4">
                                        <div class="flex items-start justify-between mb-6">
                                            <div>
                                                <h1 class="text-2xl font-bold text-gray-900">
                                                    {vm_name}
                                                </h1>
                                                <p class="text-gray-500 font-mono text-sm mt-1">
                                                    {vm_id_display}
                                                </p>
                                            </div>
                                            <span class=state_class>
                                                {state_text}
                                            </span>
                                        </div>

                                        <div class="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
                                            <div class="space-y-4">
                                                <div>
                                                    <h3 class="text-sm font-medium text-gray-500">"vCPU Count"</h3>
                                                    <p class="text-lg font-semibold text-gray-900">{vcpu_display}</p>
                                                </div>
                                                <div>
                                                    <h3 class="text-sm font-medium text-gray-500">"Memory"</h3>
                                                    <p class="text-lg font-semibold text-gray-900">{mem_display}</p>
                                                </div>
                                            </div>
                                            <div class="space-y-4">
                                                <div>
                                                    <h3 class="text-sm font-medium text-gray-500">"Console Socket"</h3>
                                                    <p class="font-mono text-sm text-gray-700 break-all">{console_path}</p>
                                                </div>
                                                <div>
                                                    <h3 class="text-sm font-medium text-gray-500">"Log Path"</h3>
                                                    <p class="font-mono text-sm text-gray-700 break-all">{log_path_display}</p>
                                                </div>
                                            </div>
                                        </div>

                                        <div class="pt-6 border-t border-gray-100">
                                            <h3 class="text-sm font-medium text-gray-500 mb-3">"Actions"</h3>
                                            <VmActions
                                                vm_id=vm_id
                                                state=vm_state
                                                on_action=handle_action
                                                loading=action_loading.get()
                                            />
                                        </div>
                                    </div>
                                }.into_any()
                            }
                            Err(e) => {
                                view! {
                                <div class="text-center py-12 mt-4">
                                    <p class="text-red-500 text-lg">"Error: " {e}</p>
                                    <a
                                        href="/"
                                        class="mt-4 inline-block px-4 py-2 text-sm font-medium text-white bg-sky-600 hover:bg-sky-700 rounded-lg"
                                    >
                                        "Back to Dashboard"
                                    </a>
                                </div>
                            }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
