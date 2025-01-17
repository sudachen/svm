use std::fmt;

use super::{to_wasm_buffer, wasm_buffer_data, BUF_ERROR_MARKER};

/// Given an error (implements `std::fmt::Debug`), allocates a Wasm buffer
/// and stores in it the printable `String` of the error (prefixed with `Error Marker`)
pub fn into_error_buffer<T: fmt::Display>(err: T) -> usize {
    let msg: String = format!("{}", err);
    let bytes = msg.as_bytes();

    let mut buf = Vec::with_capacity(1 + bytes.len());

    buf.push(BUF_ERROR_MARKER);
    buf.extend_from_slice(bytes);

    to_wasm_buffer(&buf)
}

/// Given an error `String`, allocates a Wasm buffer
/// and stores in it the error (prefixed with `Error Marker`)
pub unsafe fn error_as_string(buf: usize) -> String {
    let bytes = wasm_buffer_data(buf);
    assert_eq!(bytes[0], BUF_ERROR_MARKER);

    // skipping the `ERROR` marker
    let bytes = bytes[1..].to_vec();

    String::from_utf8_unchecked(bytes)
}

#[cfg(test)]
mod test {
    use thiserror::Error;

    use super::*;
    use crate::api::wasm;

    #[derive(Debug, Error)]
    #[error("Reason: {reason}")]
    struct MyError {
        reason: String,
    }

    #[test]
    fn wasm_into_error_buffer() {
        let err = MyError {
            reason: "An error has occurred...".to_string(),
        };

        let buf = into_error_buffer(err);

        let loaded = unsafe { error_as_string(buf) };
        println!("{:?}", loaded);
        assert_eq!(loaded, "Reason: An error has occurred...");

        wasm::free(buf);
    }
}
