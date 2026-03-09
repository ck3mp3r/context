use leptos_router::params::ParamsMap;
use std::collections::HashMap;

/// Build URL with query parameters, preserving existing params unless overridden
///
/// # Arguments
/// * `pathname` - The URL path (e.g., "/projects/123")
/// * `current_params` - Current query parameters from use_query_map()
/// * `updates` - Parameters to add/update (None value = remove param)
///
/// # Example
/// ```rust
/// let query = use_query_map();
/// let url = build_url_with_params(
///     location.pathname.get(),
///     query.read().clone(),
///     [("page".to_string(), Some("2".to_string()))].into()
/// );
/// navigate(&url, Default::default());
/// ```
pub fn build_url_with_params(
    pathname: String,
    current_params: ParamsMap,
    updates: HashMap<String, Option<String>>,
) -> String {
    // Convert ParamsMap (Cow<str> keys) to HashMap<String, String>
    let mut merged: HashMap<String, String> = current_params
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

    // Apply updates: Some(value) = set/update, None = remove
    for (key, value) in updates {
        match value {
            Some(v) => {
                merged.insert(key, v);
            }
            None => {
                merged.remove(&key);
            }
        }
    }

    // Build query string
    if merged.is_empty() {
        pathname
    } else {
        let query_string = merged
            .into_iter()
            .map(|(k, v)| {
                // Simple URL encoding for common characters
                let encoded = v.replace(' ', "+").replace('&', "%26").replace('=', "%3D");
                format!("{}={}", k, encoded)
            })
            .collect::<Vec<_>>()
            .join("&");
        format!("{}?{}", pathname, query_string)
    }
}
