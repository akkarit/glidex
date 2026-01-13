use leptos::prelude::*;

use crate::api;

#[component]
pub fn Header() -> impl IntoView {
    let health_status = Resource::new(
        || (),
        |_| async move { api::health_check().await.is_ok() },
    );

    // Set up polling for health check every 5 seconds
    #[cfg(feature = "hydrate")]
    {
        use leptos::task::spawn_local;
        Effect::new(move |_| {
            spawn_local(async move {
                loop {
                    gloo_timers::future::TimeoutFuture::new(5000).await;
                    health_status.refetch();
                }
            });
        });
    }

    view! {
        <header class="bg-white shadow-sm border-b border-gray-200">
            <div class="container mx-auto px-4">
                <div class="flex items-center justify-between h-16">
                    <div class="flex items-center space-x-4">
                        <a href="/" class="text-2xl font-bold text-sky-700">
                            "GlideX"
                        </a>
                        <span class="text-sm text-gray-500">
                            "VM Control Panel"
                        </span>
                    </div>
                    <div class="flex items-center space-x-2">
                        <span class="text-sm text-gray-600">"API:"</span>
                        <Suspense fallback=move || view! {
                            <span class="flex items-center">
                                <span class="w-2 h-2 bg-gray-400 rounded-full animate-pulse"></span>
                                <span class="ml-2 text-sm text-gray-500">"..."</span>
                            </span>
                        }>
                            {move || {
                                health_status.get().map(|is_healthy| {
                                    if is_healthy {
                                        view! {
                                            <span class="flex items-center">
                                                <span class="w-2 h-2 bg-green-500 rounded-full"></span>
                                                <span class="ml-2 text-sm text-green-600">"Healthy"</span>
                                            </span>
                                        }
                                    } else {
                                        view! {
                                            <span class="flex items-center">
                                                <span class="w-2 h-2 bg-red-500 rounded-full"></span>
                                                <span class="ml-2 text-sm text-red-600">"Offline"</span>
                                            </span>
                                        }
                                    }
                                })
                            }}
                        </Suspense>
                    </div>
                </div>
            </div>
        </header>
    }
}
