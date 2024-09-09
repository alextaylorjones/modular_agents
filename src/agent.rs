/// Test agent comment
use std::{sync::{atomic::AtomicBool, mpsc::channel, Arc}, thread::{self, JoinHandle}};
use crate::{agent_program::SignalSized, permit_store::{AtomicSizedType, PermitStore}, state::{CloneAgentStateMachine, State, StateChanged}};

// Move agents have non-thread-safe state information
#[derive(Debug)]
pub struct MoveAgent<Data: Send + Sync + 'static> {
    // Guaranteed data
    //_start_state: Arc<dyn State<Data>>, // State is owned (on the heap)
    data_store: Arc<Data>,
    signal: Arc<AtomicSizedType>, 
    // Data available after running
    permit_store: Option<PermitStore>,
    handle: Option<JoinHandle<AgentRunStatus>>,
    run_status: Option<AgentRunStatus>,
}
/// The core structure for creating, managing, and running agents
#[derive(Debug)] pub struct CloneAgent<Data: Send + Sync + 'static> { 
    // Guaranteed data
    start_state: Arc<dyn State<Data>>,// State is cloned (on the heap)
    data_store: Arc<Data>,
    abort: Arc<AtomicBool>,
    // Data available after running
    permit_store: Option<PermitStore>,
    handle: Option<JoinHandle<AgentRunStatus>>,
    run_status: Option<AgentRunStatus>,
}
/// Clones the references to the start state and the data store 
/// Note: Will not copy over any abort information or any thread-specific details
impl<Data: Send + Sync> Clone for CloneAgent<Data> {
    fn clone(&self) -> Self {
        Self { 
            start_state: self.start_state.clone(), 
            data_store: self.data_store.clone(), 
            abort: Arc::new(AtomicBool::new(false)), 
            permit_store: None, 
            handle: None, 
            run_status: None
        }
    }
}
#[derive(Debug, PartialEq)] pub enum AgentRunStatus {
    FailedToStart(String),
    FailedToJoinAndRecievePermitStore(String),
    FailedToRecievePermitStore(String),
    Success
}
#[derive(Debug, PartialEq)] pub enum AgentThreadStatus {
    NonStarted,
    RunningNotJoined,
    Joined
}
#[derive(Debug)] pub struct AgentRef<Data> {
    data: Arc<Data>,
    permit: PermitStore
}

impl<Data> AgentRef<Data> {
    pub fn new(data: Arc<Data>, permit: PermitStore)->Self {
        AgentRef {
            data,
            permit
        }
    }
}
impl<T> AgentRef<T> {
    pub fn clone_data(&self)->Arc<T> {
        self.data.clone()
    }
}
impl<T> StateChanged for AgentRef<T> {
    fn state_changed(&self) {
        self.permit.release_permit();
    }
}
impl<Data> Clone for AgentRef<Data> {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), permit: self.permit.clone() }
    }
}
impl<T: Send + Sync> StateChanged for CloneAgent<T> {
    fn state_changed(&self) {
        if let Some(permit) = &self.permit_store {
            permit.release_permit();
        }
    }
}
impl<Data: Send + Sync> MoveAgent<Data> {
    pub fn new(data_store: Arc<Data>)->Self {
        MoveAgent { 
            //start_state: start_state, 
            data_store, 
            signal: Arc::new(AtomicSizedType::new(SignalSized::MIN | SignalSized:: MAX)),
            permit_store: None,
            handle: None,
            run_status: None
        }
    }
    pub fn get_ref(&self)->Option<AgentRef<Data>>{
        if let Some(permit_store) = &self.permit_store {
            return Some(AgentRef { data: self.get_data(), permit: permit_store.clone() });
        }
        None
    }
    pub fn get_data(&self)->Arc<Data>{
        self.data_store.clone()
    }
    // Zero out the algebra
    fn mark_for_abort(&self)->SignalSized {
        crate::agent_program::AtomicSignalAlgebra::acquire_and(&self.signal,0)
    }
    /// Mark the agent for abort and wake up the scheduler if it exists
    pub fn abort(&self){
        self.mark_for_abort();
        if let Some(permit) = &self.permit_store {
            permit.release_permit();
        }
    }
    /// Returns `true` if the all the 
    pub fn has_valid_handle(&self)->bool{
        self.handle.is_some() && self.run_status.is_none() && self.permit_store.is_some()
    }
    pub fn get_thread_status(&self)-> AgentThreadStatus {
        match &self.handle {
            Some(handle) => {
                match handle.is_finished() {
                    true => AgentThreadStatus::Joined,
                    false => AgentThreadStatus::RunningNotJoined,
                } 
            },
            None => {
                AgentThreadStatus::NonStarted
            },
        }
    }
    pub fn finish(self)->Result<AgentRunStatus,String>{
        if let Some(handle) = self.handle {
            match handle.join() {
                Ok(val) => {
                    Ok(val)
                },
                Err(err) => {
                    let s = format!("Error joining: {:?}",&err);
                    Err(s)
                },
            }
        }
        else {
            let s = format!("Nothing to join. Run status is: {:?}",&self.run_status);
            Err(s)
        }
    }
}

impl<T: Send + Sync> StateChanged for MoveAgent<T> {
    /// Release by OR'ing the local signal with the input signal
    fn state_changed(&self) {
        if let Some(permit) = &self.permit_store {
            permit.release_permit();
        }
    }
}
impl<Data: Send + Sync + 'static> CloneAgent<Data> {
    pub fn new(start_state: Arc<dyn State<Data>>, data_store: Arc<Data>)->Self{
        CloneAgent { 
            start_state, 
            data_store, 
            abort: Arc::new(AtomicBool::new(false)),
            permit_store: None,
            handle: None,
            run_status: None
        }
    }
    pub fn new_arc(start_state: Arc<dyn State<Data>>, data_store: Arc<Data>)->Arc<Self>{
        Arc::new(CloneAgent { 
            start_state, 
            data_store, 
            abort: Arc::new(AtomicBool::new(false)),
            permit_store: None,
            handle: None,
            run_status: None
        })
    }
    /// Get a clone of the agent's data
    pub fn get_data(&self)->Arc<Data>{
        self.data_store.clone()
    }
    /// Get a reference to the agent
    pub fn get_ref(&self)->Option<AgentRef<Data>>{
        if let Some(permit_store) = &self.permit_store {
            return Some(AgentRef { data: self.get_data(), permit: permit_store.clone() });
        }
        None
    }
    /// Mark the agent for abort -- the scheduler will not run again (pending current finish)
    fn mark_for_abort(&self){
        self.abort.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    /// Mark the agent for abort and wake up the scheduler
    pub fn abort(&self){
        self.mark_for_abort();
        self.state_changed();
    }
    /// Determines if the agent holds a valid thread
    pub fn has_valid_handle(&self)->bool{
        self.handle.is_some() && self.run_status.is_none() && self.permit_store.is_some()
    }
    /// Checks if the scheduler thread is running (exists, but is not joined). Do not build synchronization around this.
    pub fn get_thread_status(&self)-> AgentThreadStatus {
        match &self.handle {
            Some(handle) => {
                match handle.is_finished() {
                    true => AgentThreadStatus::Joined,
                    false => AgentThreadStatus::RunningNotJoined,
                } 
            },
            None => {
                AgentThreadStatus::NonStarted
            },
        }
    }
    /// Immediately tries to finish the agent thread
    pub fn finish(self)->Result<AgentRunStatus,String>{
        if let Some(handle) = self.handle {
            match handle.join() {
                Ok(val) => {
                    Ok(val)
                },
                Err(err) => {
                    let s = format!("Error joining: {:?}",&err);
                    Err(s)
                },
            }
        }
        else {
            let s = format!("Nothing to join. Run status is: {:?}",&self.run_status);
            Err(s)
        }
    }
    // pub fn run_mover(&mut self) {
    //     let (tx,rx) = channel();
    //     // Clone resources we will send to the new thread
    //     let abort_signal = self.abort.clone();
    //     let data = self.data_store.clone();
    //     let start = self.start_state.clone();
    //     let handle = thread::spawn(move || {
    //         let mut asm = MoveAgentStateMachine::new(start);
    //         let permits = PermitStore::new(0);
    //         // Send clone of the permit store for synchronization with message system
    //         match tx.send(permits.clone()) {
    //             Ok(_) => {},
    //             Err(err) => {
    //                 return AgentRunStatus::FailedToStart(err.to_string());
    //             },
    //         }
    //         while !abort_signal.load(std::sync::atomic::Ordering::Relaxed) {
    //             permits.acquire_permit();
    //             // Advance the state (calls the scheduler on the current state)
    //             let mut next_state = asm.advance_state_move(&data);
    //             // We continue to call the scheduler as long as the scheduler returns a non-empty state
    //             while next_state && !abort_signal.load(std::sync::atomic::Ordering::Relaxed){
    //                 // Advance the state
    //                 next_state = asm.advance_state_move(&data);
    //             }
    //             // Check the state to see if it's the special abort state.
    //             // If we are in the abort state and we got here, then `next_state` is None, and we can set the abort signal
    //             if asm.is_abort_state() {
    //                 // we don't want to require `self` here so that we don't have to move `self` across threads
    //                 // otherwise this needs to be synced with `self.mark_for_abort()`
    //                 abort_signal.store(true, std::sync::atomic::Ordering::Relaxed)
    //             }
    //         }
    //         // Start the scheduler
    //         AgentRunStatus::Success
    //     });
    //     // Store a copy of the permit store
    //     match rx.recv() {
    //         // Successfully receieved the permit store
    //         Ok(permit_store) => {
    //             self.permit_store = Some(permit_store);
    //             self.handle = Some(handle);
    //         },
    //         // Failed to sync permit store between this (caller) and agent scheduler thread
    //         Err(err) => {
    //             match handle.join() {
    //                 Ok(val) => {
    //                     let s = format!("Join succeeded with status {:?}, but failed to recieve permit store: Error = {:?}",&val,&err);
    //                     self.run_status = Some(AgentRunStatus::FailedToRecievePermitStore(s));
    //                 },
    //                 Err(err) => {
    //                     let s = format!("Failed to recieve permit store: Error = {:?}",err);
    //                     self.run_status = Some(AgentRunStatus::FailedToJoinAndRecievePermitStore(s));
    //                 },
    //             }
    //             self.mark_for_abort();
    //             self.handle = None;
    //         },
    //     }
    // }
    /// Creates the agent's work thread and start the scheduler in cloning-mode
    pub fn run_clone(&mut self) {
        let (tx,rx) = channel();
        // Clone resources we will send to the new thread
        let abort_signal = self.abort.clone();
        let data = self.data_store.clone();
        let start = self.start_state.clone();

        let handle = thread::spawn(move || {
            let mut asm = CloneAgentStateMachine::new(start);
            let permits = PermitStore::new(0);
            // Send clone of the permit store for synchronization with message system
            match tx.send(permits.clone()) {
                Ok(_) => {},
                Err(err) => {
                    return AgentRunStatus::FailedToStart(err.to_string());
                },
            }
            while !abort_signal.load(std::sync::atomic::Ordering::Relaxed) {
                permits.acquire_permit();
                // Advance the state (calls the scheduler on the current state)
                let mut next_state = asm.advance_state_clone(&data);
                // We continue to call the scheduler as long as the scheduler returns a non-empty state
                while next_state && !abort_signal.load(std::sync::atomic::Ordering::Relaxed){
                    // Advance the state
                    next_state = asm.advance_state_clone(&data);
                }
                // Check the state to see if it's the special abort state.
                // If we are in the abort state and we got here, then `next_state` is None, and we can set the abort signal
                if asm.is_abort_state() {
                    // we don't want to require `self` here so that we don't have to move `self` across threads
                    // otherwise this needs to be synced with `self.mark_for_abort()`
                    abort_signal.store(true, std::sync::atomic::Ordering::Relaxed)
                }
            }

            // Start the scheduler
            AgentRunStatus::Success
        });
        // Store a copy of the permit store
        match rx.recv() {
            // Successfully receieved the permit store
            Ok(permit_store) => {
                self.permit_store = Some(permit_store);
                self.handle = Some(handle);
            },
            // Failed to sync permit store between this (caller) and agent scheduler thread
            Err(err) => {
                match handle.join() {
                    Ok(val) => {
                        let s = format!("Join succeeded with status {:?}, but failed to recieve permit store: Error = {:?}",&val,&err);
                        self.run_status = Some(AgentRunStatus::FailedToRecievePermitStore(s));
                    },
                    Err(err) => {
                        let s = format!("Failed to recieve permit store: Error = {:?}",err);
                        self.run_status = Some(AgentRunStatus::FailedToJoinAndRecievePermitStore(s));
                    },
                }
                self.mark_for_abort();
                self.handle = None;
            },
        }
    }
}
