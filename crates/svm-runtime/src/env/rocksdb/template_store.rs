use std::{marker::PhantomData, path::Path};

use svm_kv::{rocksdb::Rocksdb, traits::RawKV};
use svm_types::{AuthorAddr, Template, TemplateAddr};

use crate::env::traits::TemplateStore;
use crate::env::hash::TemplateHash;

use svm_codec::serializers::{TemplateDeserializer, TemplateSerializer};

use log::info;

/// `Template` store backed by `rocksdb`
pub struct RocksdbTemplateStore<S, D> {
    db: Rocksdb,
    phantom: PhantomData<(S, D)>,
}

impl<S, D> RocksdbTemplateStore<S, D>
where
    S: TemplateSerializer,
    D: TemplateDeserializer,
{
    /// Creates a new template store at the given path
    pub fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            db: Rocksdb::new(path),
            phantom: PhantomData,
        }
    }
}

impl<S, D> TemplateStore for RocksdbTemplateStore<S, D>
where
    S: TemplateSerializer,
    D: TemplateDeserializer,
{
    fn store(
        &mut self,
        template: &Template,
        author: &AuthorAddr,
        addr: &TemplateAddr,
        hash: &TemplateHash,
    ) {
        info!("Storing `Template`: \n{:?}", template);
        info!("     Account Address: {:?}", addr.inner());
        info!("     Hash: {:?}", hash);

        let bytes = S::serialize(template, author);

        // template addr -> code-hash
        let entry1 = (addr.inner().as_slice(), &hash.0[..]);

        // code-hash -> code
        let entry2 = (&hash.0[..], &bytes[..]);

        self.db.set(&[entry1, entry2]);
    }

    fn load(&self, addr: &TemplateAddr) -> Option<(Template, AuthorAddr)> {
        let addr = addr.inner().as_slice();

        info!("Loading `Template` account {:?}", addr);

        self.db.get(addr).and_then(|hash| {
            self.db
                .get(&hash)
                .and_then(|bytes| D::deserialize(&bytes[..]))
        })
    }
}
