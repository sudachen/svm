//! Reusable implementors of [`serde::Serialize`] and [`serde::Deserialize`].

use core::slice::SlicePattern;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use svm_types::{Address, TemplateAddr};

use super::JsonSerdeUtils;

/// A blob of binary data that is encoded via Base16.
#[derive(Clone, Debug)]
pub struct HexBlob<T>(pub T);

pub struct Addressable<const N: usize>([u8; N]);

impl<const N: usize> Addressable<N> {
    pub const fn len() -> usize {
        N
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl From<&Address> for Addressable {
    fn from(addr: &Address) -> Self {
        Self(Address::from(addr.bytes()))
    }
}

impl From<&TemplateAddr> for Addressable {
    fn from(addr: &TemplateAddr) -> Self {
        Self(Addressable(addr.bytes()))
    }
}

impl<const N: usize> Serialize for Addressable<N> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let blob = HexBlob(self.as_slice());
        blob.serialize(s)
    }
}

impl<'de, const N: usize> Deserialize<'de> for Addressable<N> {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let blob = HexBlob::deserialize(de)?;

        if blob.len() != N {
            Err(D::Error::custom("Bad length"))
        } else {
            Ok(Self(Address::from(&blob.0[..])))
        }
    }
}

impl<T> Serialize for HexBlob<T>
where
    T: AsRef<[u8]>,
{
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(hex::encode_upper(&self.0).as_str())
    }
}

impl<'de> Deserialize<'de> for HexBlob<Vec<u8>> {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let s: String = Deserialize::deserialize(de)?;
        hex::decode(s)
            .map(|bytes| Self(bytes))
            .map_err(|_| D::Error::custom("Bad hex"))
    }
}

impl Serialize for Addressable {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(s)
    }
}

impl<'de, const N: usize> Deserialize<'de> for Addressable<N> {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let blob = HexBlob::deserialize(de)?;

        if blob.0.len() != TemplateAddr::len() {
            Err(D::Error::custom("Bad length"))
        } else {
            Ok(Self(TemplateAddr::from(&blob[..])))
        }
    }
}

impl JsonSerdeUtils for Address {}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EncodedData {
    pub data: HexBlob<Vec<u8>>,
}

impl JsonSerdeUtils for EncodedData {}
