use std::cell::RefCell;
use std::rc::Rc;

use svm_common::{Address, State};
use svm_kv::memory::MemKVStore;

use crate::{default::DefaultPageCache, memory::MemAppPages, testing};

/// Initialises a new page-cache backed by a new initialized in-memory pages-storage.
pub fn app_page_cache_init(
    addr: &str,
    pages_count: u16,
) -> (
    Address,
    Rc<RefCell<MemKVStore>>,
    DefaultPageCache<MemAppPages>,
) {
    let (addr, kv, pages) = testing::app_pages_init(addr, pages_count);

    let cache = DefaultPageCache::new(pages, pages_count);

    (addr, kv, cache)
}

/// Initialises a new page-cache backed by an existing in-memory pages-storage.
pub fn app_page_cache_open(
    addr: &Address,
    state: &State,
    kv: &Rc<RefCell<MemKVStore>>,
    pages_count: u16,
) -> DefaultPageCache<MemAppPages> {
    let pages = testing::app_pages_open(addr, state, kv, pages_count);

    DefaultPageCache::new(pages, pages_count)
}
