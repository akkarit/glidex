use leptos::prelude::*;

#[component]
pub fn Loading() -> impl IntoView {
    view! {
        <div class="flex justify-center items-center py-12">
            <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-sky-600"></div>
            <span class="ml-3 text-gray-600">"Loading..."</span>
        </div>
    }
}

#[component]
pub fn LoadingCard() -> impl IntoView {
    view! {
        <div class="bg-white rounded-xl shadow-md p-6 border border-gray-100 animate-pulse">
            <div class="flex items-start justify-between">
                <div class="flex-1">
                    <div class="h-6 bg-gray-200 rounded w-1/3 mb-2"></div>
                    <div class="h-4 bg-gray-200 rounded w-2/3"></div>
                </div>
                <div class="h-6 bg-gray-200 rounded-full w-16"></div>
            </div>
            <div class="mt-4 grid grid-cols-2 gap-4">
                <div class="h-4 bg-gray-200 rounded"></div>
                <div class="h-4 bg-gray-200 rounded"></div>
            </div>
            <div class="mt-4 pt-4 border-t border-gray-100">
                <div class="flex space-x-2">
                    <div class="h-8 bg-gray-200 rounded w-16"></div>
                    <div class="h-8 bg-gray-200 rounded w-16"></div>
                </div>
            </div>
        </div>
    }
}
