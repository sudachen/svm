use byteorder::{ByteOrder, LittleEndian};

use svm_layout::Id;

use crate::FuncEnv;

macro_rules! store_n_impl {
    ($nbytes:expr, $env:ident, $mem_ptr:expr, $var_id:expr) => {{
        use svm_layout::Id;

        let bytes: Vec<u8> = {
            let borrow = $env.borrow();
            let memory = borrow.memory();
            let start = $mem_ptr as usize;
            let end = start + $nbytes;
            let view = &memory.view::<u8>()[start..end];

            view.iter().map(|cell| cell.get()).collect()
        };
        assert_eq!(bytes.len(), $nbytes);

        let mut borrow = $env.borrow_mut();
        let storage = borrow.storage_mut();
        storage.write_var(Id($var_id), bytes);
    }};
}

macro_rules! load_n_impl {
    ($nbytes:expr, $env:ident, $var_id:expr, $mem_ptr:expr) => {{
        use svm_layout::Id;

        let borrow = $env.borrow();
        let storage = borrow.storage();

        let bytes = storage.read_var(Id($var_id));
        let nbytes = bytes.len();
        assert_eq!(nbytes, $nbytes);

        let borrow = $env.borrow();
        let memory = borrow.memory();
        let start = $mem_ptr as usize;
        let end = start + $nbytes;
        let view = &memory.view::<u8>()[start..end];

        for (cell, &byte) in view.iter().zip(bytes.iter()) {
            cell.set(byte);
        }
    }};
}

/// Stores memory cells `[mem_ptr, mem_ptr + 1, ..., mem_ptr + 19]` into variable `var_id`.
///
/// # Panics
///
/// Panics if variable `var_id`'s length isn't 20 bytes.
pub fn store160(env: &FuncEnv, mem_ptr: u32, var_id: u32) {
    store_n_impl!(20, env, mem_ptr, var_id);
}

/// Loads variable `var_id` data into memory cells `[mem_ptr, mem_ptr + 1, ..., mem_ptr + 19]`
///
/// Returns the variable's length.
///
/// # Panics
///
/// Panics if variable `var_id`'s length isn't 20 bytes.
pub fn load160(env: &FuncEnv, var_id: u32, mem_ptr: u32) {
    load_n_impl!(20, env, var_id, mem_ptr);
}

/// Returns the data stored by variable `var_id` as 32-bit integer.
///
/// # Panics
///
/// Panics when variable `var_id` doesn't exist or when it consumes more than 32-bit.
pub fn get32(env: &FuncEnv, var_id: u32) -> u32 {
    let borrow = env.borrow();
    let storage = borrow.storage();
    let bytes = storage.read_var(Id(var_id));
    let nbytes = bytes.len();

    assert!(nbytes <= 4);

    let num = LittleEndian::read_uint(&bytes, nbytes);

    debug_assert!(num <= std::u32::MAX as u64);

    num as u32
}

/// Sets the data of variable `var_id` to Little-Endian representation of `value`.
///
/// # Panics
///
/// Panics when variable `var_id` doesn't exist or when it consumes more than 32-bit,
/// or when it has not enough bytes to hold `value`.
pub fn set32(env: &FuncEnv, var_id: u32, value: u32) {
    let mut borrow = env.borrow_mut();
    let storage = borrow.storage_mut();
    let (_off, nbytes) = storage.var_layout(Id(var_id));

    assert!(nbytes <= 4);

    let mut buf = vec![0; nbytes as usize];
    LittleEndian::write_uint(&mut buf, value as u64, nbytes as usize);

    storage.write_var(Id(var_id), buf);
}

/// Returns the data stored by variable `var_id` as 64-bit integer.
///
/// # Panics
///
/// Panics when variable `var_id` doesn't exist or when it consumes more than 64-bit.
pub fn get64(env: &FuncEnv, var_id: u32) -> u64 {
    let borrow = env.borrow();
    let storage = borrow.storage();
    let bytes = storage.read_var(Id(var_id));
    let nbytes = bytes.len();

    assert!(nbytes <= 8);

    LittleEndian::read_uint(&bytes, nbytes)
}

/// Sets the data of variable `var_id` to Little-Endian representation of `value`.
///
/// # Panics
///
/// Panics when variable `var_id` consumes more than 64-bit,
/// or when it has not enough bytes to hold `value`.
pub fn set64(env: &FuncEnv, var_id: u32, value: u64) {
    let mut borrow = env.borrow_mut();
    let storage = borrow.storage_mut();
    let (_off, nbytes) = storage.var_layout(Id(var_id));

    assert!(nbytes <= 8);

    let mut buf = vec![0; nbytes as usize];
    LittleEndian::write_uint(&mut buf, value, nbytes as usize);

    storage.write_var(Id(var_id), buf);
}
