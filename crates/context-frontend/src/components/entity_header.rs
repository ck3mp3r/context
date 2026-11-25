use leptos::prelude::*;

use crate::components::CopyableId;

/// Shared detail-surface header: the entity id copy button beside the entity title.
#[component]
pub fn EntityHeader(id: String, title: String) -> impl IntoView {
    view! {
        <div class="flex items-start gap-3" data-entity-header=id.clone()>
            <div class="flex-shrink-0">
                <CopyableId id=id.clone()/>
            </div>
            <h2 class="flex-1 min-w-0 break-words text-2xl font-bold text-ctp-text">{title}</h2>
        </div>
    }
}
