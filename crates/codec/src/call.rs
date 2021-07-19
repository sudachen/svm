//! Encoding for `Call Account` transactions (a.k.a [`Transaction`])
//!
//! `Transaction` Raw Format
//!
//!  +--------------------------------------------+
//!  |             |                              |
//!  |  version    |         `Address`            |
//!  |  (2 bytes)  |         (20 bytes)           |
//!  |_____________|______________________________|
//!  |                                            |
//!  |            `Function` (String)             |
//!  |____________________________________________|
//!  |                |                           |
//!  |  `VerifyData`  |       `VerifyData`        |
//!  |   #length      |          (blob)           |
//!  +________________|___________________________+
//!  |              |                             |
//!  |  `CallData`  |         `CallData`          |
//!  |   #length    |           (blob)            |
//!  +______________|_____________________________+
//!
//!

use svm_types::{AccountAddr, Transaction};

use std::io::Cursor;

use crate::{calldata, version};
use crate::{Field, ParseError, ReadExt, WriteExt};

/// Encodes a raw [`Transaction`]
pub fn encode_call(tx: &Transaction, w: &mut Vec<u8>) {
    encode_version(tx, w);
    encode_target(tx, w);
    encode_func(tx, w);
    // encode_verifydata(tx, w);
    encode_calldata(tx, w);
}

/// Parsing a raw [`Transaction`].
///
/// Returns the parsed transaction as [`Transaction`] struct.
/// On failure, returns `ParseError`
pub fn decode_call(cursor: &mut Cursor<&[u8]>) -> Result<Transaction, ParseError> {
    let version = decode_version(cursor)?;
    let target = decode_target(cursor)?;
    let func_name = decode_func(cursor)?;
    // let verifydata = calldata::decode_calldata(cursor)?;
    let calldata = calldata::decode_calldata(cursor)?;

    let tx = Transaction {
        version,
        target,
        func_name,
        // verifydata,
        calldata,
    };

    Ok(tx)
}

/// Encoders

fn encode_version(tx: &Transaction, w: &mut Vec<u8>) {
    let v = &tx.version;

    version::encode_version(*v, w);
}

fn encode_target(tx: &Transaction, w: &mut Vec<u8>) {
    let addr = tx.target.inner();

    w.write_address(addr);
}

fn encode_func(tx: &Transaction, w: &mut Vec<u8>) {
    let func = &tx.func_name;

    w.write_string(func);
}

// fn encode_verifydata(tx: &Transaction, w: &mut Vec<u8>) {
//     let verifydata = &tx.verifydata;

//     calldata::encode_calldata(verifydata, w)
// }

fn encode_calldata(tx: &Transaction, w: &mut Vec<u8>) {
    let calldata = &tx.calldata;

    calldata::encode_calldata(calldata, w)
}

/// Decoders

#[inline]
fn decode_version(cursor: &mut Cursor<&[u8]>) -> Result<u16, ParseError> {
    version::decode_version(cursor)
}

fn decode_target(cursor: &mut Cursor<&[u8]>) -> Result<AccountAddr, ParseError> {
    match cursor.read_address() {
        Ok(addr) => Ok(addr.into()),
        Err(..) => Err(ParseError::NotEnoughBytes(Field::AccountAddr)),
    }
}

fn decode_func(cursor: &mut Cursor<&[u8]>) -> Result<String, ParseError> {
    match cursor.read_string() {
        Ok(Ok(func)) => Ok(func),
        Ok(Err(..)) => Err(ParseError::InvalidUTF8String(Field::Function)),
        Err(..) => Err(ParseError::NotEnoughBytes(Field::Function)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use svm_types::Address;

    #[test]
    fn encode_decode_call() {
        let tx = Transaction {
            version: 0,
            target: Address::of("@target").into(),
            func_name: "do_work".to_string(),
            // verifydata: vec![0x10, 0x0, 0x30],
            calldata: vec![0x10, 0x0, 0x30],
        };

        let mut bytes = Vec::new();
        encode_call(&tx, &mut bytes);

        let mut cursor = Cursor::new(&bytes[..]);
        let decoded = decode_call(&mut cursor).unwrap();

        assert_eq!(tx, decoded);
    }
}