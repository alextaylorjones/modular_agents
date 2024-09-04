/// Test agent comment
use std::{sync::{atomic::AtomicBool, mpsc::channel, Arc}, thread::{self, JoinHandle}};
use crate::{permit_store::PermitStore, state::{AgentStateMachine, State}};

/// The central structure for ownership of agent resources
pub struct Agent<Data: Send + Sync + 'static> {
    // Guaranteed data
    start_state: Arc<dyn State<Data>>,
    data_store: Arc<Data>,
    abort: Arc<AtomicBool>,
    // Data available after running
    permit_store: Option<PermitStore>,
    handle: Option<JoinHandle<AgentRunStatus>>,
    run_status: Option<AgentRunStatus>,
}

/// Clones the references to the start state and the data store 
/// Note: Will not copy over any abort information or any thread-specific details
impl<Data: Send + Sync> Clone for Agent<Data> {
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
#[derive(Debug, PartialEq)]
pub enum AgentRunStatus {
    FailedToStart(String),
    FailedToJoinAndRecievePermitStore(String),
    FailedToRecievePermitStore(String),
    Success
}

#[derive(Debug, PartialEq)]
pub enum AgentThreadStatus {
    NonStarted,
    RunningNotJoined,
    Joined
}

pub struct AgentRef<Data> {
    pub data: Arc<Data>,
    pub permit: PermitStore
}
impl<Data> Clone for AgentRef<Data> {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), permit: self.permit.clone() }
    }
}

impl<Data: Send + Sync + 'static> Agent<Data> {
    pub fn new(start_state: Arc<dyn State<Data>>, data_store: Arc<Data>)->Self{
        Agent { 
            start_state, 
            data_store, 
            abort: Arc::new(AtomicBool::new(false)),
            permit_store: None,
            handle: None,
            run_status: None
        }
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
    /// Give a permit to the permit store to indicate some state has changed. This will prompt the scheduler to run again, if not aborted.
    pub fn state_changed(&self)->bool{
        match &self.permit_store {
            Some(store) => {
                store.release_permit();
                true
            },
            None => {
                false
            },
        }
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
    /// Creates the agent's work thread and start the scheduler
    pub fn run(&mut self) {
        let (tx,rx) = channel();
        // Clone resources we will send to the new thread
        let abort_signal = self.abort.clone();
        let data = self.data_store.clone();
        let start = self.start_state.clone();

        let handle = thread::spawn(move || {
            let mut asm = AgentStateMachine::new(start);
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
                let mut next_state = asm.advance_state(&data);
                while next_state && !abort_signal.load(std::sync::atomic::Ordering::Relaxed){
                    // Advance the state
                    next_state = asm.advance_state(&data);
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
