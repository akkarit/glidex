use leptos::prelude::*;

#[component]
pub fn Modal(
    title: &'static str,
    #[prop(into)] on_close: Callback<()>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="fixed inset-0 z-50 overflow-y-auto">
            <div class="flex min-h-screen items-center justify-center p-4">
                // Backdrop
                <div
                    class="fixed inset-0 bg-black bg-opacity-50 transition-opacity"
                    on:click=move |_| on_close.run(())
                ></div>

                // Modal content
                <div class="relative bg-white rounded-xl shadow-xl max-w-lg w-full p-6 z-10">
                    <div class="flex items-center justify-between mb-4">
                        <h2 class="text-xl font-semibold text-gray-900">{title}</h2>
                        <button
                            class="text-gray-400 hover:text-gray-600 transition-colors"
                            on:click=move |_| on_close.run(())
                        >
                            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
                            </svg>
                        </button>
                    </div>
                    {children()}
                </div>
            </div>
        </div>
    }
}
