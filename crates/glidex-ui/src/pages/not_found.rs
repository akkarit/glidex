use leptos::prelude::*;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="flex flex-col items-center justify-center py-16">
            <h1 class="text-6xl font-bold text-gray-300">"404"</h1>
            <p class="mt-4 text-xl text-gray-600">"Page not found"</p>
            <a
                href="/"
                class="mt-6 px-4 py-2 text-sm font-medium text-white bg-sky-600 hover:bg-sky-700 rounded-lg transition-colors"
            >
                "Back to Dashboard"
            </a>
        </div>
    }
}
