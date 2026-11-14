use serde::Serialize;
use serde_json::Value;

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendInfo {
    pub version: Option<String>,
    pub node_version: Option<String>,
    pub hive_version: Option<String>,
    pub platform: Option<String>,
    pub arch: Option<String>,
}

fn field(value: &Value, key: &str) -> Option<String> {
    value[key]
        .as_str()
        .filter(|text| !text.is_empty() && text.len() <= 128 && !text.chars().any(char::is_control))
        .map(str::to_owned)
}

impl BackendInfo {
    pub fn from_initialize(info: &Value) -> Self {
        let runtime = &info["runtimeInfo"];
        Self {
            version: field(runtime, "version").or_else(|| field(info, "version")),
            node_version: field(runtime, "nodeVersion"),
            hive_version: field(runtime, "hiveVersion"),
            platform: field(runtime, "platform"),
            arch: field(runtime, "arch"),
        }
    }
}

pub fn revision() -> Option<&'static str> {
    let value = env!("HIVE_DESKTOP_GIT_REVISION");
    (value != "unknown").then_some(value)
}

pub fn dirty() -> Option<bool> {
    match env!("HIVE_DESKTOP_GIT_DIRTY") {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub fn copy_text(connection: &str, backend: Option<&BackendInfo>) -> String {
    // Copy only these explicit fields, never the whole initialize response,
    // connection errors, environment, paths, navigation or session content.
    serde_json::to_string_pretty(&serde_json::json!({
        "application": "Hive Desktop",
        "desktop": {
            "version": env!("CARGO_PKG_VERSION"),
            "revision": revision(),
            "trackedFilesModified": dirty(),
            "profile": env!("HIVE_DESKTOP_BUILD_PROFILE"),
            "target": env!("HIVE_DESKTOP_BUILD_TARGET"),
        },
        "connection": connection,
        "backend": backend,
    }))
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legacy_initialize_and_bounded_copy_exclude_private_fields() {
        let legacy = BackendInfo::from_initialize(
            &json!({"version":"0.1.0","dataDir":"private-dir","navigation":"private-navigation"}),
        );
        assert_eq!(legacy.version.as_deref(), Some("0.1.0"));
        assert!(legacy.node_version.is_none());
        let info = BackendInfo::from_initialize(&json!({"version":"0.1.0","runtimeInfo":{
            "version":"0.2.0","nodeVersion":"v22.23.3","hiveVersion":"2.2.1","platform":"darwin","arch":"arm64",
            "secret":"private-token","environment":{"TOKEN":"private-token"}
        }}));
        let copied = copy_text("connected", Some(&info));
        let parsed: Value = serde_json::from_str(&copied).unwrap();
        assert_eq!(
            parsed["backend"],
            json!({"version":"0.2.0","nodeVersion":"v22.23.3","hiveVersion":"2.2.1","platform":"darwin","arch":"arm64"})
        );
        assert_eq!(parsed.as_object().unwrap().len(), 4);
        assert!(!copied.contains("private-"));
        assert_eq!(
            serde_json::from_str::<Value>(&copy_text("disconnected", None)).unwrap()["backend"],
            Value::Null
        );
    }

    #[test]
    fn invalid_metadata_is_unavailable_without_losing_valid_fields() {
        let info = BackendInfo::from_initialize(&json!({"runtimeInfo":{
            "version":42,"nodeVersion":"x".repeat(129),"hiveVersion":"bad\nvalue","platform":"win32","arch":"x64"
        }}));
        assert!(
            info.version.is_none() && info.node_version.is_none() && info.hive_version.is_none()
        );
        assert_eq!(info.platform.as_deref(), Some("win32"));
        assert_eq!(info.arch.as_deref(), Some("x64"));
    }
}
