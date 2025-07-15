use miette::{Diagnostic, NamedSource, SourceSpan};
use std::collections::HashMap;
use std::path::Path;
use yaml_spanned::{Spanned, Value as YamlValue, from_str};

#[derive(Debug, Clone)]
pub struct SpanInfo {
    pub start: usize,
    pub end: usize,
}

#[derive(thiserror::Error, Debug, Diagnostic)]
#[error("{message}")]
pub struct ValidationError {
    #[source_code]
    pub source_code: NamedSource<String>,

    #[label("here")]
    pub span: SourceSpan,

    pub message: String,
}

pub struct SpannedValidator {
    spans: HashMap<String, SpanInfo>,
    json_value: serde_json::Value,
    source: String,
    file_path: String,
}

impl SpannedValidator {
    pub fn new(file_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let source = std::fs::read_to_string(file_path)?;
        let file_path_str = file_path.display().to_string();

        // Parse YAML with spans
        let spanned_yaml: Spanned<YamlValue> = from_str(&source)?;

        // Strip spans and build lookup map
        let mut spans = HashMap::new();
        let json_value = Self::strip_spans_and_index(&spanned_yaml, String::new(), &mut spans);

        Ok(Self {
            spans,
            json_value,
            source,
            file_path: file_path_str,
        })
    }

    pub fn get_json_value(&self) -> &serde_json::Value {
        &self.json_value
    }

    pub fn create_error(&self, instance_path: &str, message: String) -> ValidationError {
        let pointer_str = instance_path;

        // Look up span info, default to start of file if not found
        let span_info = self
            .spans
            .get(pointer_str)
            .or_else(|| self.spans.get(""))
            .cloned()
            .unwrap_or(SpanInfo { start: 0, end: 0 });

        ValidationError {
            source_code: NamedSource::new(&self.file_path, self.source.clone()),
            span: SourceSpan::new(span_info.start.into(), span_info.end - span_info.start),
            message,
        }
    }

    fn strip_spans_and_index(
        spanned: &Spanned<YamlValue>,
        path: String,
        spans: &mut HashMap<String, SpanInfo>,
    ) -> serde_json::Value {
        // Record span for this path
        let span = spanned.span();
        spans.insert(
            path.clone(),
            SpanInfo {
                start: span.start.unwrap_or_default().byte_index,
                end: span.end.unwrap_or_default().byte_index,
            },
        );

        match spanned.as_ref() {
            YamlValue::Null => serde_json::Value::Null,
            YamlValue::Bool(b) => serde_json::Value::Bool(*b),
            YamlValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    serde_json::Value::Number(serde_json::Number::from(i))
                } else if let Some(u) = n.as_u64() {
                    serde_json::Value::Number(serde_json::Number::from(u))
                } else if let Some(f) = n.as_f64() {
                    serde_json::Number::from_f64(f)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null)
                } else {
                    serde_json::Value::Null
                }
            }
            YamlValue::String(s) => serde_json::Value::String(s.clone()),
            YamlValue::Sequence(seq) => {
                let arr: Vec<serde_json::Value> = seq
                    .iter()
                    .enumerate()
                    .map(|(i, item)| {
                        let item_path = if path.is_empty() {
                            format!("/{i}")
                        } else {
                            format!("{path}/{i}")
                        };
                        Self::strip_spans_and_index(item, item_path, spans)
                    })
                    .collect();
                serde_json::Value::Array(arr)
            }
            YamlValue::Mapping(map) => {
                let mut obj = serde_json::Map::new();
                for (key_spanned, value_spanned) in map {
                    if let YamlValue::String(key) = key_spanned.as_ref() {
                        let value_path = if path.is_empty() {
                            format!("/{key}")
                        } else {
                            format!("{path}/{key}")
                        };
                        let value = Self::strip_spans_and_index(value_spanned, value_path, spans);
                        obj.insert(key.clone(), value);
                    }
                }
                serde_json::Value::Object(obj)
            }
            YamlValue::Tagged(tagged_value) => {
                Self::strip_spans_and_index(&tagged_value.value, path, spans)
            }
        }
    }
}
