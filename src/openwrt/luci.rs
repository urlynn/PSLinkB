/// ubus push + state.json fallback

#[cfg(feature = "openwrt")]
mod imp {
    use serde_json::{json, Value};

    const STATE_FILE: &str = "/tmp/pslinkb/state.json";

    pub trait IntoStateValue {
        fn into_value(self) -> Value;
    }

    impl IntoStateValue for &str {
        fn into_value(self) -> Value {
            if self.starts_with('{') {
                serde_json::from_str(self).unwrap_or(Value::String(self.to_string()))
            } else {
                Value::String(self.to_string())
            }
        }
    }

    impl IntoStateValue for &String {
        fn into_value(self) -> Value {
            self.as_str().into_value()
        }
    }

    impl IntoStateValue for bool {
        fn into_value(self) -> Value {
            Value::Bool(self)
        }
    }

    fn default_value(key: &str) -> Value {
        match key {
            "running" => Value::Bool(false),
            "qr"   => json!({ "url": "", "status": "" }),
            "live" => json!({ "status": "" }),
            "dns"  => json!({
                "checking": false,
                "enabled": false,
                "target": "",
                "actual": "",
                "ok": false
            }),
            _ => Value::String(String::new()),
        }
    }

    /// 初始空状态
    fn empty_state() -> Value {
        json!({
            "running": true,
            "user": "",
            "qr": { "url": "", "status": "" },
            "live": { "status": "" },
            "error": "",
            "dns": {
                "checking": false,
                "enabled": false,
                "target": "",
                "actual": "",
                "ok": false
            }
        })
    }

    pub fn init() {
        let _ = std::fs::create_dir_all("/tmp/pslinkb");
        write_file(&empty_state());
    }

    /// 设置一个顶层字段
    pub fn set<T: IntoStateValue>(key: &str, value: T) {
        let parsed = value.into_value();

        push_event(key, &parsed);

        let mut state = read_state();
        state[key] = parsed;
        write_file(&state);
    }

    /// 清空一个顶层字段
    pub fn clear(key: &str) {
        let empty = default_value(key);
        push_event(key, &empty);
        let mut state = read_state();
        state[key] = empty;
        write_file(&state);
    }

    pub fn read_state() -> Value {
        std::fs::read_to_string(STATE_FILE)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(empty_state)
    }

    fn write_file(state: &Value) {
        let tmp = format!("/tmp/pslinkb/.state.json.tmp");
        if std::fs::write(&tmp, state.to_string()).is_ok() {
            let _ = std::fs::rename(&tmp, STATE_FILE);
        }
    }

    fn push_event(key: &str, value: &Value) {
        let payload = json!({
            "type": "pslinkb",
            "data": {
                "key": key,
                "value": value
            }
        });
        // ubus push
        let _ = std::process::Command::new("ubus")
            .args(["call", "service", "event", &payload.to_string()])
            .spawn();
    }
}

#[cfg(feature = "openwrt")]
pub use imp::*;

#[cfg(feature = "cli")]
mod stub {
    pub fn init() {}
    pub fn set<T>(_key: &str, _value: T) {}
    pub fn clear(_key: &str) {}
    pub fn read_state() -> serde_json::Value { serde_json::Value::Null }
}

#[cfg(feature = "cli")]
pub use stub::*;
