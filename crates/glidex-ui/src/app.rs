use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment, ParamSegment,
};

use crate::components::Header;
use crate::pages::{Dashboard, NotFound, VmDetail};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body class="bg-gray-50 min-h-screen">
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/glidex-ui.css"/>
        <Title text="GlideX - VM Control Panel"/>

        <Router>
            <div class="min-h-screen">
                <Header/>
                <main class="container mx-auto px-4 py-8">
                    <Routes fallback=|| view! { <NotFound/> }>
                        <Route path=StaticSegment("") view=Dashboard/>
                        <Route path=(StaticSegment("vms"), ParamSegment("id")) view=VmDetail/>
                    </Routes>
                </main>
            </div>
        </Router>
    }
}
