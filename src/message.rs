use std::sync::{Arc, MutexGuard, PoisonError};



/// A data-consumer trait that allows single-data storage
pub trait StoresSingle<Data> {
    fn store(&self, data: Data);
}

/// A data-consumer trait the allows slice storage
pub trait StoresVec<Data> {
    fn store_slice(&self, data: &[Data]);
}
pub trait DefaultMessageError<T> {

}
pub trait MessageTarget<T> {
    fn msg(self: Arc<Self>, data: T);
}
pub trait MessageTestTarget<T, E> {
    fn msg(self: Arc<Self>, data: T)->Result<(),E>;
}
