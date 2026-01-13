use leptos::prelude::*;

use crate::api;
use crate::components::{CreateVmForm, LoadingCard, Modal, VmAction, VmCard};
use crate::types::VmResponse;

#[component]
pub fn Dashboard() -> impl IntoView {
    let (show_create_modal, set_show_create_modal) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (_action_loading, set_action_loading) = signal(false);

    // Resource for fetching VMs
    let vms_resource = LocalResource::new(move || async move { api::list_vms().await });

    // Refetch function
    let refetch = move || {
        vms_resource.refetch();
    };

    // Handle VM actions
    let handle_action = Callback::new(move |(vm_id, action): (String, VmAction)| {
        set_action_loading.set(true);
        set_error.set(None);

        #[cfg(feature = "hydrate")]
        {
            use leptos::task::spawn_local;
            spawn_local(async move {
                let result = match action {
                    VmAction::Start => api::start_vm(&vm_id).await.map(|_| ()),
                    VmAction::Stop => api::stop_vm(&vm_id).await.map(|_| ()),
                    VmAction::Pause => api::pause_vm(&vm_id).await.map(|_| ()),
                    VmAction::Delete => {
                        // In browser, we'd use web_sys::window().confirm()
                        // For now, just proceed
                        api::delete_vm(&vm_id).await
                    }
                };

                set_action_loading.set(false);

                if let Err(e) = result {
                    set_error.set(Some(e));
                }

                // Refetch VM list
                refetch();
            });
        }

        #[cfg(not(feature = "hydrate"))]
        {
            let _ = (vm_id, action);
            set_action_loading.set(false);
        }
    });

    // Handle create VM
    let handle_create = Callback::new(move |request| {
        set_error.set(None);

        #[cfg(feature = "hydrate")]
        {
            use leptos::task::spawn_local;
            spawn_local(async move {
                match api::create_vm(request).await {
                    Ok(_) => {
                        set_show_create_modal.set(false);
                        refetch();
                    }
                    Err(e) => {
                        set_error.set(Some(e));
                    }
                }
            });
        }

        #[cfg(not(feature = "hydrate"))]
        {
            let _ = request;
        }
    });

    view! {
        <div>
            // Header with title and create button
            <div class="flex items-center justify-between mb-6">
                <div>
                    <h1 class="text-2xl font-bold text-gray-900">"Virtual Machines"</h1>
                    <p class="text-gray-500 mt-1">"Manage your Firecracker VMs"</p>
                </div>
                <button
                    class="px-4 py-2 text-sm font-medium text-white bg-sky-600 hover:bg-sky-700 rounded-lg transition-colors"
                    on:click=move |_| set_show_create_modal.set(true)
                >
                    "+ Create VM"
                </button>
            </div>

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

            // VM Grid
            <Suspense fallback=move || view! {
                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                    <LoadingCard/>
                    <LoadingCard/>
                    <LoadingCard/>
                </div>
            }>
                {move || {
                    vms_resource.get().map(|result| {
                        match (*result).clone() {
                            Ok(vms) => {
                                if vms.is_empty() {
                                    view! {
                                        <div class="text-center py-12">
                                            <div class="text-gray-400 mb-4">
                                                <svg class="w-16 h-16 mx-auto" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"></path>
                                                </svg>
                                            </div>
                                            <p class="text-gray-500 text-lg">"No VMs found"</p>
                                            <p class="text-gray-400 mt-1">"Create your first VM to get started"</p>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                                            <For
                                                each=move || vms.clone()
                                                key=|vm| vm.id.clone()
                                                children=move |vm: VmResponse| {
                                                    view! {
                                                        <VmCard vm=vm on_action=handle_action/>
                                                    }
                                                }
                                            />
                                        </div>
                                    }.into_any()
                                }
                            }
                            Err(e) => {
                                view! {
                                <div class="text-center py-12">
                                    <p class="text-red-500">"Error loading VMs: " {e}</p>
                                    <button
                                        class="mt-4 px-4 py-2 text-sm font-medium text-white bg-sky-600 hover:bg-sky-700 rounded-lg"
                                        on:click=move |_| refetch()
                                    >
                                        "Retry"
                                    </button>
                                </div>
                            }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>

            // Create VM Modal
            {move || show_create_modal.get().then(|| view! {
                <Modal title="Create New VM" on_close=move || set_show_create_modal.set(false)>
                    <CreateVmForm
                        on_submit=handle_create
                        on_cancel=move || set_show_create_modal.set(false)
                    />
                </Modal>
            })}
        </div>
    }
}
