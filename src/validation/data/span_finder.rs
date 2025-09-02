use miette::SourceSpan;
use yaml_spanned::{Spanned, Value as YamlValue};

/// Helper for finding spans in spanned YAML values
pub struct SpanFinder<'a> {
    root: &'a Spanned<YamlValue>,
}

impl<'a> SpanFinder<'a> {
    pub fn new(root: &'a Spanned<YamlValue>) -> Self {
        Self { root }
    }

    /// Find the span for a nested field path
    pub fn find_field_span(&self, path: &[&str]) -> Option<SourceSpan> {
        let mut current = self.root;

        for &key in path {
            match current.as_ref() {
                YamlValue::Mapping(map) => {
                    if let Some((_, next_value)) = map.iter().find(|(k, _)| match k.as_ref() {
                        YamlValue::String(s) => s == key,
                        _ => false,
                    }) {
                        current = next_value;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        // Return span of the final value
        let span = current.span();
        Some(SourceSpan::new(
            span.start.unwrap_or_default().byte_index.into(),
            span.end.unwrap_or_default().byte_index - span.start.unwrap_or_default().byte_index,
        ))
    }

    /// Find the span for an array item with a specific value
    #[allow(dead_code)]
    pub fn find_array_item_span(&self, path: &[&str], item_value: &str) -> Option<SourceSpan> {
        let mut current = self.root;

        // Navigate to the array field
        for &key in path {
            match current.as_ref() {
                YamlValue::Mapping(map) => {
                    if let Some((_, next_value)) = map.iter().find(|(k, _)| match k.as_ref() {
                        YamlValue::String(s) => s == key,
                        _ => false,
                    }) {
                        current = next_value;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        // Find the specific item in the array
        match current.as_ref() {
            YamlValue::Sequence(seq) => {
                for item in seq {
                    match item.as_ref() {
                        YamlValue::String(s) if s == item_value => {
                            let span = item.span();
                            return Some(SourceSpan::new(
                                span.start.unwrap_or_default().byte_index.into(),
                                span.end.unwrap_or_default().byte_index
                                    - span.start.unwrap_or_default().byte_index,
                            ));
                        }
                        _ => continue,
                    }
                }
            }
            _ => return None,
        }

        None
    }

    /// Find the span for a cache reference (which can be in different formats)
    #[allow(dead_code)]
    pub fn find_cache_reference_span(&self, cache_name: &str) -> Option<SourceSpan> {
        // Look in restore_cache array
        if let Some(restore_cache) = self.find_field_value(&["restore_cache"])
            && let YamlValue::Sequence(seq) = restore_cache.as_ref()
        {
            for item in seq {
                match item.as_ref() {
                    // Simple string reference
                    YamlValue::String(s) if s == cache_name => {
                        let span = item.span();
                        return Some(SourceSpan::new(
                            span.start.unwrap_or_default().byte_index.into(),
                            span.end.unwrap_or_default().byte_index
                                - span.start.unwrap_or_default().byte_index,
                        ));
                    }
                    // Complex object reference
                    YamlValue::Mapping(map) => {
                        if let Some((_, name_value)) = map.iter().find(|(k, _)| match k.as_ref() {
                            YamlValue::String(s) => s == "name",
                            _ => false,
                        }) && let YamlValue::String(s) = name_value.as_ref()
                            && s == cache_name
                        {
                            let span = name_value.span();
                            return Some(SourceSpan::new(
                                span.start.unwrap_or_default().byte_index.into(),
                                span.end.unwrap_or_default().byte_index
                                    - span.start.unwrap_or_default().byte_index,
                            ));
                        }
                    }
                    _ => continue,
                }
            }
        }

        None
    }

    /// Helper to find a field value in spanned YAML
    #[allow(dead_code)]
    pub fn find_field_value(&self, path: &[&str]) -> Option<&Spanned<YamlValue>> {
        let mut current = self.root;

        for &key in path {
            match current.as_ref() {
                YamlValue::Mapping(map) => {
                    if let Some((_, next_value)) = map.iter().find(|(k, _)| match k.as_ref() {
                        YamlValue::String(s) => s == key,
                        _ => false,
                    }) {
                        current = next_value;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(current)
    }
}
