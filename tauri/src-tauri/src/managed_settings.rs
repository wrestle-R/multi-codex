//! Edit only migration-owned settings; rollback preserves subsequent user edits.
use crate::profiles::Result;
use jsonc_parser::cst::{CstInputValue, CstRootNode};
use jsonc_parser::ParseOptions;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingChange {
    pub key: String,
    pub previous: Option<Value>,
    pub installed: Value,
}

fn input(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(v) => CstInputValue::Bool(*v),
        Value::Number(v) => CstInputValue::Number(v.to_string()),
        Value::String(v) => CstInputValue::String(v.clone()),
        Value::Array(v) => CstInputValue::Array(v.iter().map(input).collect()),
        Value::Object(v) => {
            CstInputValue::Object(v.iter().map(|(k, v)| (k.clone(), input(v))).collect())
        }
    }
}

fn parse(text: &str) -> Result<(CstRootNode, BTreeMap<String, Value>)> {
    let root = CstRootNode::parse(text, &ParseOptions::default())
        .map_err(|_| "Editor settings contain invalid JSONC; the legacy layout is retained")?;
    let object = root
        .object_value()
        .ok_or("Editor settings must contain a JSON object")?;
    let mut seen = HashSet::new();
    let mut values = BTreeMap::new();
    for property in object.properties() {
        let key = property
            .decoded_name()
            .ok_or("Invalid editor setting name")?;
        if !seen.insert(key.clone()) {
            return Err("Duplicate editor setting keys must be resolved before migration".into());
        }
        let raw = property
            .value()
            .ok_or("Editor setting has no value")?
            .to_string();
        let value: Value =
            jsonc_parser::parse_to_serde_value::<Value>(&raw, &ParseOptions::default())
                .map_err(|_| "Editor setting value is invalid")?;
        values.insert(key, value);
    }
    Ok((root, values))
}

pub fn install(
    text: &str,
    settings: &BTreeMap<String, Value>,
) -> Result<(String, Vec<SettingChange>)> {
    let (root, current) = parse(text)?;
    let object = root.object_value().unwrap();
    let mut changes = Vec::new();
    for (key, installed) in settings {
        if current.get(key) == Some(installed) {
            continue;
        }
        changes.push(SettingChange {
            key: key.clone(),
            previous: current.get(key).cloned(),
            installed: installed.clone(),
        });
        if let Some(property) = object.get(key) {
            property.set_value(input(installed));
        } else {
            object.append(key, input(installed));
        }
    }
    Ok((root.to_string(), changes))
}

pub fn rollback(text: &str, changes: &[SettingChange]) -> Result<String> {
    let (root, current) = parse(text)?;
    let object = root.object_value().unwrap();
    for change in changes {
        // A deliberate edit after migration belongs to the user, including removal.
        if current.get(&change.key) != Some(&change.installed) {
            continue;
        }
        if let Some(property) = object.get(&change.key) {
            if let Some(previous) = &change.previous {
                property.set_value(input(previous));
            } else {
                property.remove();
            }
        }
    }
    Ok(root.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comments_and_unrelated_values_survive_install_and_rollback() {
        let text = "{\n // my theme\n \"workbench.colorTheme\": \"Custom\",\n // keep my Copilot state\n \"github.copilot.enable\": {\"*\": false},\n \"extensions.autoUpdate\": true,\n}";
        let desired = BTreeMap::from([
            ("extensions.autoUpdate".into(), Value::Bool(false)),
            ("chatgpt.openOnStartup".into(), Value::Bool(true)),
        ]);
        let (installed, changes) = install(text, &desired).unwrap();
        assert!(installed.contains("// my theme"));
        assert!(installed.contains("// keep my Copilot state"));
        let user_edit = installed.replace("\"Custom\"", "\"New theme\"");
        let restored = rollback(&user_edit, &changes).unwrap();
        let (_, values) = parse(&restored).unwrap();
        assert_eq!(values["workbench.colorTheme"], "New theme");
        assert_eq!(values["github.copilot.enable"]["*"], false);
        assert_eq!(values["extensions.autoUpdate"], true);
        assert!(!values.contains_key("chatgpt.openOnStartup"));
    }
    #[test]
    fn rollback_keeps_subsequent_edits_of_managed_values() {
        let desired = BTreeMap::from([(
            "chatgpt.cliExecutable".into(),
            Value::String("managed".into()),
        )]);
        let (installed, changes) = install("{}", &desired).unwrap();
        let edited = installed.replace("managed", "custom");
        assert_eq!(rollback(&edited, &changes).unwrap(), edited);
    }
    #[test]
    fn malformed_non_object_and_duplicate_settings_are_rejected() {
        for text in ["[]", "{broken}", "{\"a\":1,\"a\":2}"] {
            assert!(install(text, &BTreeMap::new()).is_err());
        }
    }
}
