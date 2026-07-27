use std::collections::HashMap;

use greplog_core::redact::{redact_string, RedactionMode};

struct Rule {
    patterns: &'static [&'static str],
    mode: RedactionMode,
}

fn rules() -> &'static [Rule] {
    &[
        Rule { patterns: &["password"], mode: RedactionMode::Full },
        Rule { patterns: &["token"], mode: RedactionMode::Full },
        Rule { patterns: &["secret"], mode: RedactionMode::Full },
        Rule { patterns: &["email"], mode: RedactionMode::Partial },
    ]
}

pub fn redact_attributes(attrs: &HashMap<String, String>) -> HashMap<String, String> {
    let mut result = HashMap::with_capacity(attrs.len());
    for (key, val) in attrs {
        let key_lower = key.to_lowercase();
        let mut redacted = val.clone();
        for rule in rules() {
            if rule.patterns.iter().any(|p| key_lower.contains(p)) {
                redacted = redact_string(val, rule.mode);
                break;
            }
        }
        result.insert(key.clone(), redacted);
    }
    result
}
