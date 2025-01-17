//! Encoding of a binary [`SpawnAccount`].
//!
//! ```text
//!
//!  +-----------+-------------+----------------+
//!  |           |             |                |
//!  |  Version  |  Template   |      Name      |
//!  |   (u16)   |  (Address)  |    (String)    |
//!  |           |             |                |
//!  +-----------+-------------+----------------+
//!  |           |                              |
//!  |   Ctor    |          CallData            |
//!  |  (String) |           (Blob)             |
//!  |           |                              |
//!  +-----------+------------------------------+
//!
//! ```

use std::io::Cursor;

use svm_types::{Account, SpawnAccount, TemplateAddr};

use crate::{inputdata, version};
use crate::{Field, ParseError, ReadExt, WriteExt};

/// Encodes a binary [`SpawnAccount`] transaction.
pub fn encode(spawn: &SpawnAccount, w: &mut Vec<u8>) {
    encode_version(spawn, w);
    encode_template(spawn, w);
    encode_name(spawn, w);
    encode_ctor(spawn, w);
    encode_ctor_calldata(spawn, w);
}

/// Parsing a binary [`SpawnAccount`] transaction.
///
/// Returns the parsed [`SpawnAccount`],
/// On failure, returns [`ParseError`].
pub fn decode(cursor: &mut Cursor<&[u8]>) -> Result<SpawnAccount, ParseError> {
    let version = decode_version(cursor)?;
    let template_addr = decode_template(cursor)?;
    let name = decode_name(cursor)?;
    let ctor_name = decode_ctor(cursor)?;
    let calldata = decode_ctor_calldata(cursor)?;

    let account = Account {
        name,
        template_addr,
    };

    let spawn = SpawnAccount {
        version,
        account,
        ctor_name,
        calldata,
    };

    Ok(spawn)
}

/// Encoders

fn encode_version(spawn: &SpawnAccount, w: &mut Vec<u8>) {
    let v = &spawn.version;
    version::encode_version(*v, w);
}

fn encode_name(spawn: &SpawnAccount, w: &mut Vec<u8>) {
    let name = spawn.account_name();
    w.write_string(name);
}

fn encode_template(spawn: &SpawnAccount, w: &mut Vec<u8>) {
    let template = spawn.template_addr();
    w.write_template_addr(template);
}

fn encode_ctor(spawn: &SpawnAccount, w: &mut Vec<u8>) {
    let ctor = spawn.ctor_name();
    w.write_string(ctor);
}

fn encode_ctor_calldata(spawn: &SpawnAccount, w: &mut Vec<u8>) {
    let calldata = &*spawn.calldata;
    inputdata::encode_inputdata(calldata, w);
}

/// Decoders

#[inline]
fn decode_version(cursor: &mut Cursor<&[u8]>) -> Result<u16, ParseError> {
    version::decode_version(cursor)
}

fn decode_template(cursor: &mut Cursor<&[u8]>) -> Result<TemplateAddr, ParseError> {
    cursor
        .read_template_addr()
        .map_err(|_| ParseError::NotEnoughBytes(Field::Address))
}

fn decode_name(cursor: &mut Cursor<&[u8]>) -> Result<String, ParseError> {
    match cursor.read_string() {
        Ok(Ok(name)) => Ok(name),
        Ok(Err(..)) => Err(ParseError::InvalidUTF8String(Field::Name)),
        Err(..) => Err(ParseError::NotEnoughBytes(Field::Name)),
    }
}

fn decode_ctor(cursor: &mut Cursor<&[u8]>) -> Result<String, ParseError> {
    match cursor.read_string() {
        Ok(Ok(ctor)) => Ok(ctor),
        Ok(Err(..)) => Err(ParseError::InvalidUTF8String(Field::Ctor)),
        Err(..) => Err(ParseError::NotEnoughBytes(Field::Ctor)),
    }
}

fn decode_ctor_calldata(cursor: &mut Cursor<&[u8]>) -> Result<Vec<u8>, ParseError> {
    inputdata::decode_inputdata(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    use svm_types::TemplateAddr;

    #[test]
    fn encode_decode_spawn() {
        let spawn = SpawnAccount {
            version: 0,
            account: Account {
                name: "@account".to_string(),
                template_addr: TemplateAddr::of("@template"),
            },
            ctor_name: "initialize".to_string(),
            calldata: vec![0x10, 0x20, 0x30],
        };

        let mut bytes = Vec::new();
        encode(&spawn, &mut bytes);

        let mut cursor = Cursor::new(&bytes[..]);
        let decoded = decode(&mut cursor).unwrap();

        assert_eq!(spawn, decoded);
    }
}
