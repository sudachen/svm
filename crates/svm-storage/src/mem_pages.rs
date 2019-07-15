use super::{DefaultPageHasher, DefaultPagesStorage, MemKVStore};

/// `MemPages` is a pages-storage backed by an in-memory key-value store (`MemKVStore`)
pub type MemPages<K> = DefaultPagesStorage<DefaultPageHasher, MemKVStore<K>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::PagesStorage;
    use std::cell::RefCell;
    use std::rc::Rc;
    use svm_common::Address;

    #[test]
    fn a_page_does_not_exit_by_default() {
        let addr = Address::from(0x11_22_33_44 as u32);

        let kv = Rc::new(RefCell::new(MemKVStore::new()));
        let mut storage = MemPages::new(addr, kv);

        assert_eq!(None, storage.read_page(0));
    }

    #[test]
    fn writing_a_page_does_not_auto_commit_it_to_underlying_kv() {
        let addr = Address::from(0x11_22_33_44 as u32);

        let kv = Rc::new(RefCell::new(MemKVStore::new()));
        let kv_clone = Rc::clone(&kv);

        // both `storage1` and `storage2` service the same contract address `addr`
        // and both share the the same underlying key-value store
        let mut storage1 = MemPages::new(addr, kv);
        let mut storage2 = MemPages::new(addr, kv_clone);

        // writing `page 0` with data `[10, 20, 30]`
        // changes aren't commited directly to `kv`
        storage1.write_page(0, &vec![10, 20, 30]);
        assert_eq!(None, storage1.read_page(0));
        assert_eq!(None, storage2.read_page(0));

        // another assertion for the uncommitted changes
        assert_eq!(1, storage1.uncommitted_len());
        assert_eq!(0, storage2.uncommitted_len());

        // now, storage `storage1` commits pending changes to `kv`
        storage1.commit();

        // both `storage1` and `storage2` report the same persisted `page 0`
        assert_eq!(vec![10, 20, 30], storage1.read_page(0).unwrap());
        assert_eq!(vec![10, 20, 30], storage2.read_page(0).unwrap());

        // no more pending changes
        assert_eq!(0, storage1.uncommitted_len());
        assert_eq!(0, storage2.uncommitted_len());
    }

    #[test]
    fn writing_the_same_page_twice_before_committing() {
        let addr = Address::from(0x11_22_33_44 as u32);

        let kv = Rc::new(RefCell::new(MemKVStore::new()));
        let mut storage = MemPages::new(addr, kv);

        // first write
        storage.write_page(0, &vec![10, 20, 30]);
        // one pending change
        assert_eq!(1, storage.uncommitted_len());

        // second write (page-override)
        storage.write_page(0, &vec![40, 50, 60]);
        // still, one pending change
        assert_eq!(1, storage.uncommitted_len());

        // commit page
        storage.commit();

        assert_eq!(vec![40, 50, 60], storage.read_page(0).unwrap());
        // no pending changes
        assert_eq!(0, storage.uncommitted_len());
    }

    #[test]
    fn committing_the_same_page_under_two_different_contract_addresses() {
        let addr1 = Address::from(0x11_22_33_44 as u32);
        let addr2 = Address::from(0x55_66_77_88 as u32);

        let kv = Rc::new(RefCell::new(MemKVStore::new()));
        let kv_clone = Rc::clone(&kv);

        // `storagee1` and `storage2` share the same underlying `kv store`
        let mut storage1 = MemPages::new(addr1, kv);
        let mut storage2 = MemPages::new(addr2, kv_clone);

        storage1.write_page(0, &vec![10, 20, 30]);
        storage2.write_page(0, &vec![40, 50, 60]);

        // committing pending changes
        storage1.commit();
        storage2.commit();

        // no more pending changes
        assert_eq!(0, storage1.uncommitted_len());
        assert_eq!(0, storage2.uncommitted_len());

        // two pages `[10, 20, 30]` and `[40, 50, 60]` have been committed successfully
        assert_eq!(vec![10, 20, 30], storage1.read_page(0).unwrap());
        assert_eq!(vec![40, 50, 60], storage2.read_page(0).unwrap());
    }
}
