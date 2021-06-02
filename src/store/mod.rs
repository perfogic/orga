use crate::error::Result;
use std::ops::{Bound, Deref, DerefMut, RangeBounds};

pub mod bufstore;
pub mod nullstore;
pub mod rwlog;
pub mod share;
pub mod store;

pub use bufstore::{Map as BufStoreMap, BufStore, MapStore};
pub use nullstore::NullStore;
pub use rwlog::RWLog;
pub use share::Shared;
pub use store::{Store, DefaultBackingStore};

// TODO: Key type (for cheaper concat, enum over ref or owned slice, etc)

pub type KV = (Vec<u8>, Vec<u8>);

/// Trait for read access to key/value stores.
pub trait Read {
    /// Gets a value by key.
    ///
    /// Implementations of `get` should return `None` when there is no value for
    /// the key rather than erroring.
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    fn get_next(&self, key: &[u8]) -> Result<Option<KV>>;

    fn get_prev(&self, key: &[u8]) -> Result<Option<KV>>;

    #[inline]
    fn range<B: RangeBounds<Vec<u8>>>(&self, bounds: B) -> Iter<Self> {
        todo!()
    }
}

impl<R: Read, T: Deref<Target = R>> Read for T {
    #[inline]
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.deref().get(key)
    }

    #[inline]
    fn get_next(&self, key: &[u8]) -> Result<Option<KV>> {
        self.deref().get_next(key)
    }

    #[inline]
    fn get_prev(&self, key: &[u8]) -> Result<Option<KV>> {
        self.deref().get_prev(key)
    }
}

/// Trait for write access to key/value stores.
pub trait Write: Read {
    /// Writes a key and value to the store.
    ///
    /// If a value already exists for the given key, implementations should
    /// overwrite the value.
    fn put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<()>;

    /// Deletes the value with the given key.
    ///
    /// If no value exists for the given key, implementations should treat the
    /// operation as a no-op (but may still issue a call to `delete` to an
    /// underlying store).
    fn delete(&mut self, key: &[u8]) -> Result<()>;
}

impl<S: Write, T: DerefMut<Target = S>> Write for T {
    #[inline]
    fn put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        self.deref_mut().put(key, value)
    }

    #[inline]
    fn delete(&mut self, key: &[u8]) -> Result<()> {
        self.deref_mut().delete(key)
    }
}

// TODO: make reversible
pub struct Iter<'a, S: ?Sized> {
    parent: &'a S,
    key: Vec<u8>,
    end: Bound<Vec<u8>>,
}

impl<'a, S: Read> Iterator for Iter<'a, S> {
    type Item = KV;
    
    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

// #[cfg(test)]
// mod tests {
//     use super::{NullStore, Read};
//     use crate::state::Value;

//     #[test]
//     fn fixed_length_slice_key() {
//         let key = b"0123";
//         NullStore.get(key).unwrap();
//     }

//     #[test]
//     fn slice_key() {
//         let key = vec![1, 2, 3, 4];
//         NullStore.get(key.as_slice()).unwrap();
//     }

//     #[test]
//     fn wrap() {
//         let _: Value<_, u64> = NullStore.wrap().unwrap();
//     }
// }
