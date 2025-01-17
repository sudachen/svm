use serde_json::{json, Value};

use svm_sdk::template;

#[template]
mod Template {
    #[fundable]
    #[ctor]
    fn init() {}

    #[fundable_hook(default)]
    fn fund() {
        //
    }
}

fn main() {
    let raw = raw_meta();
    let json: Value = serde_json::from_str(&raw).unwrap();

    assert_eq!(
        json,
        json!({
            "schema": [],
            "api": [json!({
                "name": "init",
                "wasm_name": "init",
                "doc": "",
                "is_ctor": true,
                "is_fundable": true,
                "signature": json!({"params": [], "returns": {}}),
            })],
        })
    );
}
