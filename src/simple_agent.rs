use std::{sync::{Arc, Condvar, Mutex}, thread::{self, JoinHandle}};

use crate::permit_store::PermitStore;

#[derive(PartialEq)]
pub enum RunState {
    GoAgain,
    ReqNewPermit,
    Finish
}
#[derive(Clone)]
pub struct SimpleAgent<'a, Data: Send + 'static> {
    data: Data,
    func: fn(&mut Self)->RunState
}

impl<'a, Data: Send + 'static> SimpleAgent<'a, Data> {
    pub fn new(data: Data, func: fn(&mut Self)->RunState)->Self {
        SimpleAgent {
            data,
            func
        }
    }
}

pub trait RunSimple {
    fn run(data: Self)-> Option<(thread::JoinHandle<Self>, crate::permit_store::PermitStore)> where Self: Sized;
}

pub type Scheduler<T> = fn(&mut SimpleAgent<T>)->RunState;

// TODO: Function should return/create a message interface
pub fn run_ref<T: Send>(mut state: SimpleAgent<'static, T>)->Option<(JoinHandle<T>, PermitStore)>{
    let pair: Arc<(Mutex<Option<PermitStore>>,Condvar)> = Arc::new((Mutex::new(None), Condvar::new()));
    let pair2 = pair.clone();

    let handle = thread::spawn(move || {
        let permits = PermitStore::new(0);
        {
            let (lock, cvar) = &*pair2;
            let mut started = lock.lock().unwrap();
            *started = Some(permits.clone());
            //println!("Gave the permits");
            // We notify the condvar that the value has changed.
            cvar.notify_one();            
        } // drop the lock
        
        permits.acquire_permit();
        let mut status;
        'outer : {
            loop {
                status = (state.func)(&mut state);
                match status {
                    RunState::GoAgain => {},
                    RunState::ReqNewPermit => {
                        permits.acquire_permit();
                    },
                    RunState::Finish => {
                        break 'outer;
                    },
                }
            }
        
        }
        return state.data;                
    });
    
    
    let (lock, cvar) = &*pair;
    
    {
        // Wait for the thread to start up.
        let mut started = lock.lock().unwrap();
        while started.is_none() {
            started = cvar.wait(started).unwrap();
        }
        match started.clone() {
            Some(guard) => {
                return Some((handle, guard));
            },
            None => {
                None
            },
        }
    }
    
}

pub fn run<T: Send>(mut state: SimpleAgent<'static, T>)->Option<(JoinHandle<T>, PermitStore)>{
    let pair: Arc<(Mutex<Option<PermitStore>>,Condvar)> = Arc::new((Mutex::new(None), Condvar::new()));
    let pair2 = pair.clone();

    let handle = thread::spawn(move || {
        let permits = PermitStore::new(0);
        {
            let (lock, cvar) = &*pair2;
            let mut started = lock.lock().unwrap();
            *started = Some(permits.clone());
            //println!("Gave the permits");
            // We notify the condvar that the value has changed.
            cvar.notify_one();            
        } // drop the lock
    
        permits.acquire_permit();
        let mut status;
        'outer : {
            loop {
                status = (state.func)(&mut state);
                match status {
                    RunState::GoAgain => {},
                    RunState::ReqNewPermit => {
                        permits.acquire_permit();
                    },
                    RunState::Finish => {
                        break 'outer;
                    },
                }
            }
        
        }
        return state.data;                
    });
    
    // Wait for the thread to start up.
    let (lock, cvar) = &*pair;
    
    //println!("Waiting for lock");
    {
        let mut started = lock.lock().unwrap();
        while started.is_none() {
            started = cvar.wait(started).unwrap();
        }
        //println!("Got past permitting in run");
        match started.clone() {
            Some(guard) => {
                return Some((handle, guard));
            },
            None => {
                None
            },
        }
    }
    
}

#[cfg(test)]
mod test{
    use std::{sync::{atomic::AtomicUsize, Arc, RwLock}, thread, time::Instant, usize};


    use crate::simple_agent::{run, RunSimple, RunState, Scheduler, SimpleAgent};

    pub mod setup_it_works {
        use crate::simple_agent::RunSimple;

        use super::*;
        pub struct MyTestAgent {
            pub val: usize
        }

        impl MyTestAgent {
            const FINISH_SCHEDULER: Scheduler<MyTestAgent> = |_| {
                //println!("Finish scheduler ({})", &this.data.val);
                RunState::Finish
            };
            
            const SECOND_SCHEDULER: Scheduler<MyTestAgent> = |this: &mut SimpleAgent<MyTestAgent>| {
                println!("Second scheduler ({})", &this.data.val);
                if this.data.val < 20 {
                    this.data.val += 1;
                    RunState::GoAgain
                } else {
                    this.func = Self::INIT_SCHEDULER;
                    RunState::ReqNewPermit
                }
                
            };

            const INIT_SCHEDULER: Scheduler<MyTestAgent> = |this: &mut SimpleAgent<MyTestAgent>| {
                println!("Init scheduler ({})", &this.data.val);
                
                if this.data.val < 10 {
                    this.data.val += 1;
                } else if this.data.val  == 10 {
                    this.func = Self::SECOND_SCHEDULER;
                } else if this.data.val < 30 {
                    this.data.val += 1;
                } else if this.data.val == usize::MAX {
                    panic!("forced panic");
                }else {
                    this.func = Self::FINISH_SCHEDULER;
                }
                RunState::GoAgain
            };
        }


        impl RunSimple for MyTestAgent {
            fn run(data: MyTestAgent)-> Option<(thread::JoinHandle<MyTestAgent>, crate::permit_store::PermitStore)>{
                let s= SimpleAgent::new(data, Self::INIT_SCHEDULER);
                run(s)
            }
        }
    }
    pub mod setup_shared_data {
        use std::sync::{Arc, RwLock};

        use super::*;
        pub struct MyTestAgent {
            pub val: Arc<RwLock<usize>>
        }

        impl MyTestAgent {
            const FINISH_SCHEDULER: Scheduler<MyTestAgent> = |_| {
                //println!("Finish scheduler ({})", &this.data.val);
                RunState::Finish
            };
            
            const SECOND_SCHEDULER: Scheduler<MyTestAgent> = |this: &mut SimpleAgent<MyTestAgent>| {
                if *this.data.val.read().unwrap() < 20 {
                    *this.data.val.write().unwrap() += 1;
                    RunState::GoAgain
                } else {
                    this.func = Self::INIT_SCHEDULER;
                    RunState::ReqNewPermit
                }
                
            };

            const INIT_SCHEDULER: Scheduler<MyTestAgent> = |this: &mut SimpleAgent<MyTestAgent>| {
                let n = *this.data.val.read().unwrap();
                if n  < 10 {
                    *this.data.val.write().unwrap() += 1;
                } else if n  == 10 {
                    this.func = Self::SECOND_SCHEDULER;
                } else if n < 30 {
                    *this.data.val.write().unwrap() += 1;
                } else if n == usize::MAX {
                    {
                        let r = this.data.val.write();
                        panic!("forced panic with write guard");
                    }
                } else {
                    this.func = Self::FINISH_SCHEDULER;
                }
                RunState::GoAgain
            };
        }


        impl RunSimple for MyTestAgent {
            fn run(data: MyTestAgent)-> Option<(thread::JoinHandle<MyTestAgent>, crate::permit_store::PermitStore)>{
                let s= SimpleAgent::new(data, Self::INIT_SCHEDULER);
                run(s)
            }
        }
    }
    pub mod setup_message_simple {
        use std::sync::{atomic::AtomicUsize, Arc, RwLock};

        use super::*;
        pub struct MyTestAgent {
            pub finish_value: usize,
            pub shared_reader: Arc<RwLock<usize>>
        }
        pub struct MyTestAgentAtomic {
            pub finish_value: usize,
            pub shared_atomic: Arc<AtomicUsize>
        }

        impl MyTestAgent {
            const FINISH_SCHEDULER: Scheduler<MyTestAgent> = |_| {
                RunState::Finish
            };

            const INIT_SCHEDULER: Scheduler<MyTestAgent> = |this: &mut SimpleAgent<MyTestAgent>| {
                let n = *this.data.shared_reader.read().unwrap();
                if n  < this.data.finish_value {
                    *this.data.shared_reader.write().unwrap() += 1;
                } else {
                    this.func = Self::FINISH_SCHEDULER;
                }
                RunState::GoAgain
            };
        }

        impl MyTestAgentAtomic {
            const FINISH_SCHEDULER: Scheduler<MyTestAgentAtomic> = |_| {
                RunState::Finish
            };

            const INIT_SCHEDULER: Scheduler<MyTestAgentAtomic> = |this: &mut SimpleAgent<MyTestAgentAtomic>| {
                let n = this.data.shared_atomic.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if n  < this.data.finish_value {
                    
                } else {
                    this.data.shared_atomic.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    this.func = Self::FINISH_SCHEDULER;
                }
                RunState::GoAgain
            };
        }

        impl RunSimple for MyTestAgent {
            fn run(data: MyTestAgent)-> Option<(thread::JoinHandle<MyTestAgent>, crate::permit_store::PermitStore)>{
                let s= SimpleAgent::new(data, Self::INIT_SCHEDULER);
                run(s)
            }
        }
        impl RunSimple for MyTestAgentAtomic {
            fn run(data: MyTestAgentAtomic)-> Option<(thread::JoinHandle<MyTestAgentAtomic>, crate::permit_store::PermitStore)>{
                let s= SimpleAgent::new(data, Self::INIT_SCHEDULER);
                run(s)
            }
        }
    }
    #[test] fn it_works(){
        
        use setup_it_works::MyTestAgent;

        let my_data = MyTestAgent {val: 0};
        let agent_handle = MyTestAgent::run(my_data);
        //println!("Finishing running agent");
        assert!(agent_handle.is_some());
        let (handle, agent_ref) = agent_handle.unwrap();
        //println!("Waiting to release");
        agent_ref.release_permit();
        
        assert!(!handle.is_finished());
        //println!("Should be stuck at 20, and go into inti scheduler until 30");
        agent_ref.release_permit();
        assert!(handle.join().unwrap().val == 30);
    }
    #[test] fn it_works_recover_panic(){
        use setup_it_works::MyTestAgent;

        let my_data = MyTestAgent {val: usize::MAX};
        let agent_handle = MyTestAgent::run(my_data);
        println!("Finishing running agent");
        assert!(agent_handle.is_some());
        let (handle, agent_ref) = agent_handle.unwrap();
        println!("Waiting to release");
        agent_ref.release_permit();
        match handle.join() {
            Ok(_) => assert!(false, "We should not get any data back!"),
            Err(_) => assert!(true, "We should get an error"),
        }
    }
    #[test] fn rwlock_recover_panic(){
        use setup_shared_data::MyTestAgent;
        let val = Arc::new(RwLock::new(usize::MAX));
        let my_data = MyTestAgent { val: val.clone() };
        let agent_handle = MyTestAgent::run(my_data);
        println!("Finishing running agent");
        assert!(agent_handle.is_some());
        let (handle, agent_ref) = agent_handle.unwrap();
        println!("Waiting to release");
        agent_ref.release_permit();
        match handle.join() {
            Ok(_) => assert!(false, "We should not get any data back!"),
            Err(_) => assert!(true, "We should get an error"),
        }
        assert!(val.is_poisoned());
    }
    #[test] fn simple_agent_shared(){
        const NUM_AGENTS: usize = 11;
        use setup_message_simple::MyTestAgent;
        let finish_value = 1_000_000;
        let shared_reader = Arc::new(RwLock::new(0));
        let mut handles = Vec::new();
        let starttime = Instant::now();
        for _ in 0..NUM_AGENTS {
            let my_data = MyTestAgent {shared_reader: shared_reader.clone() , finish_value};
            let agent_handle = MyTestAgent::run(my_data);
            assert!(agent_handle.is_some());
            let (handle, agent_ref) = agent_handle.unwrap();
            agent_ref.release_permit();
            handles.push(handle);
        }

        println!("Startup time {:?}", Instant::now() - starttime);
        let starttime = Instant::now();
        for _ in 0..NUM_AGENTS {
            let handle = handles.pop();
            if let Some(handle) = handle {
                let res = *handle.join().unwrap().shared_reader.read().unwrap();
                assert!(res <= NUM_AGENTS + finish_value);
            }
        }
        println!("Runtime {:?}", Instant::now() - starttime);
    }
    #[test] fn simple_agent_shared_atomic(){
        const NUM_AGENTS: usize = 11;
        use setup_message_simple::MyTestAgentAtomic;
        let finish_value = 1_000_000;
        let shared_reader = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        let starttime = Instant::now();
        for _ in 0..NUM_AGENTS {
            let my_data = MyTestAgentAtomic {shared_atomic: shared_reader.clone() , finish_value};
            let agent_handle = MyTestAgentAtomic::run(my_data);
            assert!(agent_handle.is_some());
            let (handle, agent_ref) = agent_handle.unwrap();
            agent_ref.release_permit();
            handles.push(handle);
        }

        println!("Startup time {:?}", Instant::now() - starttime);
        let starttime = Instant::now();
        for _ in 0..NUM_AGENTS {
            let handle = handles.pop();
            if let Some(handle) = handle {
                let res = handle.join().unwrap().shared_atomic.load(std::sync::atomic::Ordering::Relaxed);
                assert!(res == finish_value);
            }
        }
        println!("Runtime {:?}", Instant::now() - starttime);
    }
    

}