use super::wasm_buf_apply;
use crate::api::{self, json::JsonError};

/// Encodes a `Deploy Template` json input into SVM a binary format.
/// The json input is passed by giving WASM memory start address (`ptr` parameter).
///
/// Returns a pointer to a `transaction buffer`.
///
/// See also: `alloc` and `free`
pub fn encode_deploy(ptr: usize) -> Result<usize, JsonError> {
    wasm_buf_apply(ptr, api::json::deploy_template)
}

#[cfg(test)]
mod test {
    use super::*;

    use std::io::Cursor;
    use std::vec;

    use svm_layout::Layout;
    use svm_types::{CodeKind, CodeSection, CtorsSection, DataSection, GasMode, HeaderSection};

    use crate::api::builder::TemplateBuilder;
    use crate::api::wasm::{
        error_as_string, free, to_wasm_buffer, wasm_buffer_data, BUF_OK_MARKER,
    };

    use crate::template;

    #[test]
    fn wasm_deploy_valid() {
        let json = r#"{
          "name": "My Template",
          "desc": "A few words",
          "code": "C0DE",
          "svm_version": 1,
          "code_version": 2,
          "data": "0000000100000003",
          "ctors": ["init", "start"]
        }"#;

        let json_buf = to_wasm_buffer(json.as_bytes());
        let tx_buf = encode_deploy(json_buf).unwrap();

        let data = wasm_buffer_data(tx_buf);
        assert_eq!(data[0], BUF_OK_MARKER);

        let cursor = Cursor::new(&data[1..]);
        let actual = template::decode(cursor, None).unwrap();

        let code = CodeSection::new(
            CodeKind::Wasm,
            vec![0xC0, 0xDE],
            CodeSection::exec_flags(),
            GasMode::Fixed,
            1,
        );
        let data = DataSection::with_layout(Layout::Fixed(vec![1, 3].into()));
        let ctors = CtorsSection::new(vec!["init".into(), "start".into()]);
        let header = HeaderSection::new(2, "My Template".into(), "A few words".into());

        let expected = TemplateBuilder::default()
            .with_code(code)
            .with_data(data)
            .with_ctors(ctors)
            .with_header(header)
            .build();

        assert_eq!(actual, expected);

        free(json_buf);
        free(tx_buf);
    }

    #[test]
    fn wasm_deploy_invalid() {
        let json = "{";

        let json_buf = to_wasm_buffer(json.as_bytes());
        let error_buf = encode_deploy(json_buf).unwrap();

        let error = unsafe { error_as_string(error_buf) };

        assert_eq!(error, "The given JSON is syntactically invalid due to EOF.");

        free(json_buf);
        free(error_buf);
    }
}
