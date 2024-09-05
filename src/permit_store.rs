#![forbid(unsafe_code)]
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;
use std::thread::{self, Thread};

/// A thread-safe permit store. Allows multiple client threads to release permits, and a single owner thread to acquire permits.
#[derive(Clone, Debug)]
pub struct PermitStore {
    permits: Arc<AtomicIsize>,
    owner_thread: Thread,
}

impl PermitStore {
    /// Creates a new permit store with `init_permits` free permits, associated with the current (calling) thread.
    pub fn new(init_permits: isize)->Self {
        PermitStore { permits: Arc::new(AtomicIsize::new(init_permits)), owner_thread: thread::current()}
    }
    /// Released a permit to the store. Can be called by any thread.
    pub fn release_permit(&self) {
        // Add a permit to the store
        self.permits.fetch_add(1, Ordering::Relaxed);
        // Unpark the owner thread (or add an unpark to future parks)
        self.owner_thread.unpark();
    }
    /// Waits for a free permit from the permit store. Blocks until one is available.
    /// Should be called inside the thread that instantiated the store.
    pub fn acquire_permit(&self) {
        // Optional check -- make sure that only the owner thread can acquire the state
        if thread::current().id() != self.owner_thread.id() {
            panic!("can't use state this way");
        }
        while self.permits.load(Ordering::Relaxed) <= 0 {
            thread::park();
        }
        self.permits.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod test {
    use std::thread;
    use super::PermitStore;
    /// Tests whether we can start the permit store with multiple permits
    #[test]
    fn local_access(){
        const NUM_PERMITS: isize = 5;
        let permits = PermitStore::new(NUM_PERMITS);
        for _ in 0..NUM_PERMITS {
            permits.acquire_permit();
        }
    }
    /// Tests whether a single thread can release permits
    #[test]
    fn single_thread_access(){
        let permits = PermitStore::new(0);
        let handle;
        {
            let permits = permits.clone();
            handle = thread::spawn(move || {
                permits.release_permit();
            });
        }
        permits.acquire_permit();
        assert!(handle.join().is_ok());
    }
    /// Tests whether multiple client threads can release permits
    #[test]
    fn multi_thread_access(){
        const NUM_THREADS: usize = 10;
        let permits = PermitStore::new(0);
        let mut handles = Vec::new();
        for _ in 0..NUM_THREADS {
            let permits = permits.clone();
            handles.push(thread::spawn(move || {
                permits.release_permit();
            }));
        }

        for _ in 0..NUM_THREADS { permits.acquire_permit(); }

        let _ = handles.into_iter().map(|h| assert!(h.join().is_ok()));
    }
    /// Test whether a single client can release multiple permits
    #[test]
    fn single_thread_multi_permit() {
        const NUM_THREADS: usize = 10;
        let permits = PermitStore::new(0);
        let handle;
        {
            let permits = permits.clone();
            handle = thread::spawn(move || {
                for _ in 0..NUM_THREADS {
                    permits.release_permit();
                }
            });
        }
        for _ in 0..NUM_THREADS {
            permits.acquire_permit();
        }        
        assert!(handle.join().is_ok());
    }
    /// Test whether multiple clients can release multiple permits
    #[test]
    fn multi_thread_multi_permit(){
        const NUM_THREADS: usize = 10;
        const NUM_PERMITS_PER_THREAD: usize = 10;
        let permits = PermitStore::new(0);
        let mut handles = Vec::new();
        for _ in 0..NUM_THREADS {
            let permits = permits.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..NUM_PERMITS_PER_THREAD {
                    permits.release_permit();
                }
            }));
        }

        for _ in 0..NUM_THREADS*NUM_PERMITS_PER_THREAD { permits.acquire_permit(); }

        let _ = handles.into_iter().map(|h| assert!(h.join().is_ok()));
    }
}