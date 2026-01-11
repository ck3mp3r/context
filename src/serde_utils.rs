/// Serde utility functions for common patterns
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::marker::PhantomData;

/// Deserialize `Option<Option<T>>` to distinguish between missing field and null value.
///
/// - Missing field → `None`
/// - Field is `null` → `Some(None)`
/// - Field has value → `Some(Some(value))`
///
/// Usage:
/// ```ignore
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Example {
///     #[serde(default, deserialize_with = "crate::serde_utils::double_option")]
///     parent_id: Option<Option<String>>,
/// }
/// ```
pub fn double_option<'de, T, D>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    struct DoubleOptionVisitor<T> {
        _inner: PhantomData<T>,
    }

    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for DoubleOptionVisitor<T> {
        type Value = Option<Option<T>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("option")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(None))
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            T::deserialize(deserializer).map(|val| Some(Some(val)))
        }
    }

    de.deserialize_option(DoubleOptionVisitor {
        _inner: PhantomData,
    })
}
