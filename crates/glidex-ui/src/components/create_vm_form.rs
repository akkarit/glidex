use leptos::prelude::*;

use crate::types::CreateVmRequest;

#[component]
pub fn CreateVmForm(
    #[prop(into)] on_submit: Callback<CreateVmRequest>,
    #[prop(into)] on_cancel: Callback<()>,
) -> impl IntoView {
    let (name, set_name) = signal(String::new());
    let (vcpu_count, set_vcpu_count) = signal(1u8);
    let (mem_size_mib, set_mem_size_mib) = signal(512u32);
    let (kernel_path, set_kernel_path) = signal(String::new());
    let (rootfs_path, set_rootfs_path) = signal(String::new());
    let (kernel_args, set_kernel_args) = signal(String::new());
    let (submitting, set_submitting) = signal(false);

    let default_kernel = "~/.glidex/vmlinux.bin".to_string();
    let default_rootfs = "~/.glidex/rootfs.ext4".to_string();

    let handle_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_submitting.set(true);

        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());

        let kernel = if kernel_path.get().is_empty() {
            format!("{}/.glidex/vmlinux.bin", home)
        } else {
            kernel_path.get()
        };

        let rootfs = if rootfs_path.get().is_empty() {
            format!("{}/.glidex/rootfs.ext4", home)
        } else {
            rootfs_path.get()
        };

        let request = CreateVmRequest {
            name: name.get(),
            vcpu_count: vcpu_count.get(),
            mem_size_mib: mem_size_mib.get(),
            kernel_image_path: kernel,
            rootfs_path: rootfs,
            kernel_args: if kernel_args.get().is_empty() {
                None
            } else {
                Some(kernel_args.get())
            },
        };

        on_submit.run(request);
    };

    view! {
        <form on:submit=handle_submit class="space-y-4">
            <div>
                <label class="block text-sm font-medium text-gray-700">"VM Name"</label>
                <input
                    type="text"
                    class="mt-1 w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-sky-500 focus:border-transparent"
                    placeholder="my-vm"
                    required
                    prop:value=move || name.get()
                    on:input=move |ev| set_name.set(event_target_value(&ev))
                />
            </div>

            <div class="grid grid-cols-2 gap-4">
                <div>
                    <label class="block text-sm font-medium text-gray-700">"vCPU Count"</label>
                    <input
                        type="number"
                        class="mt-1 w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-sky-500 focus:border-transparent"
                        min="1"
                        max="32"
                        prop:value=move || vcpu_count.get().to_string()
                        on:input=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse() {
                                set_vcpu_count.set(v);
                            }
                        }
                    />
                </div>
                <div>
                    <label class="block text-sm font-medium text-gray-700">"Memory (MiB)"</label>
                    <input
                        type="number"
                        class="mt-1 w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-sky-500 focus:border-transparent"
                        min="128"
                        max="32768"
                        prop:value=move || mem_size_mib.get().to_string()
                        on:input=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse() {
                                set_mem_size_mib.set(v);
                            }
                        }
                    />
                </div>
            </div>

            <div>
                <label class="block text-sm font-medium text-gray-700">"Kernel Image Path"</label>
                <input
                    type="text"
                    class="mt-1 w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-sky-500 focus:border-transparent"
                    placeholder=default_kernel.clone()
                    prop:value=move || kernel_path.get()
                    on:input=move |ev| set_kernel_path.set(event_target_value(&ev))
                />
                <p class="mt-1 text-xs text-gray-500">"Leave empty for default"</p>
            </div>

            <div>
                <label class="block text-sm font-medium text-gray-700">"Root Filesystem Path"</label>
                <input
                    type="text"
                    class="mt-1 w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-sky-500 focus:border-transparent"
                    placeholder=default_rootfs.clone()
                    prop:value=move || rootfs_path.get()
                    on:input=move |ev| set_rootfs_path.set(event_target_value(&ev))
                />
                <p class="mt-1 text-xs text-gray-500">"Leave empty for default"</p>
            </div>

            <div>
                <label class="block text-sm font-medium text-gray-700">"Kernel Arguments (optional)"</label>
                <input
                    type="text"
                    class="mt-1 w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-sky-500 focus:border-transparent"
                    placeholder="console=ttyS0 reboot=k panic=1 pci=off"
                    prop:value=move || kernel_args.get()
                    on:input=move |ev| set_kernel_args.set(event_target_value(&ev))
                />
            </div>

            <div class="flex justify-end space-x-3 pt-4">
                <button
                    type="button"
                    class="px-4 py-2 text-sm font-medium text-gray-700 bg-gray-200 hover:bg-gray-300 rounded-lg transition-colors"
                    on:click=move |_| on_cancel.run(())
                >
                    "Cancel"
                </button>
                <button
                    type="submit"
                    class="px-4 py-2 text-sm font-medium text-white bg-sky-600 hover:bg-sky-700 rounded-lg transition-colors disabled:opacity-50"
                    disabled=move || submitting.get()
                >
                    {move || if submitting.get() { "Creating..." } else { "Create VM" }}
                </button>
            </div>
        </form>
    }
}
