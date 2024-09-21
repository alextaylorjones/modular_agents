use std::{fmt::Debug, marker::PhantomData, sync::{atomic::Ordering, Arc, Condvar, Mutex}, thread::{spawn, JoinHandle}};

use crate::{agent::{AgentRef, AgentRunStatus, AgentThreadStatus, CloneAgent}, permit_store::{AtomicSizedType, PermitStore}, state::{MoveAgentStateMachine, State, StateChanged}};


pub trait FromConfigClone<Config> {
    fn from_config(c: &Config)->Arc<Self>;
    fn start_state(c: &Config)->Arc<dyn State<Self>>;
}

pub trait FromConfigMove<Config> {
    fn from_config(c: Config)->Arc<Self>;
    fn start_state()->Arc<dyn State<Self>>;
}

pub struct AtomicSignalAlgebra {
    _signal: Arc<AtomicSizedType>
}

impl AtomicSignalAlgebra {
    pub fn acquire_and(signal: &Arc<AtomicSizedType> , a: SignalSized)->SignalSized {
        signal.fetch_and(a, Ordering::Relaxed)
    }
    pub fn release_or(signal: &Arc<AtomicSizedType> , r: SignalSized)-> SignalSized {
        signal.fetch_or(r, Ordering::Relaxed)
    }
}
pub struct MoveAgentProgram<Config, Data: Send + Sync + 'static> {
    agent_thread: JoinHandle<Arc<Data>>,
    agent_ref: AgentRef<Data>,
    signal_atomic_algebra: Arc<AtomicSizedType>,
    _phantom: PhantomData<Config>
}
pub struct Signal {
     
}

#[cfg(feature = "word")]
pub type SignalSized = isize;
impl Signal {
    pub const ABORT: isize = isize::MAX;
}


impl<Config: Send + 'static, Data: Debug + Send + Sync + 'static + FromConfigMove<Config>> MoveAgentProgram<Config, Data> {
    pub fn is_thread_finished(&self)->bool{
        self.agent_thread.is_finished()
    }
    /// Create & run a new agent from config
    pub fn run_move(config: Config)->Result<MoveAgentProgram<Config, Data>, AgentProgramError<Self>> where Config: Debug {
        println!("Starting MoveAgentProgramm, in run_move");
        let permit_condvar: Arc<(Mutex<Option<PermitStore>>,Condvar)> = Arc::new((Mutex::new(None), Condvar::new()));
        let permit_condvar_cloned = permit_condvar.clone();
        let data_condvar: Arc<(Mutex<Option<Arc<Data>>>,Condvar)> = Arc::new((Mutex::new(None), Condvar::new()));
        let data_condvar_cloned = data_condvar.clone();
        let abort_atomic = Arc::new(AtomicSizedType::new(0)); // All 1's for signed complement
        let signal = abort_atomic.clone();
        let handle = spawn(move || {
            println!("In new thread {:?}",&config);
            // Create the initial data from the config
            let data: Arc<Data> = Data::from_config(config);
            {
                let (lock, cvar) = &*data_condvar_cloned;
                match lock.lock() {
                    Ok(mut guard) => {
                        *guard = Some(data.clone());
                    },
                    Err(err) => {
                        println!("Permit store mutex was poisoned {:?}", &err);
                        return data;
                    }
                }
                
                cvar.notify_one();            
            } // drop the data lock
            

            // Load permits and notify caller.
            let permits = PermitStore::new(0);
            {
                // Get a local referenece to the condition variable tuple.
                let (lock, cvar) = &*permit_condvar_cloned;
                //
                match lock.lock() {
                    Ok(mut guard) => {
                        println!("Dropped permit lock {:?} [{:?}]", &permits, &data);
                        *guard = Some(permits.clone());
                    },
                    Err(err) => {
                        println!("Permit store mutex was poisoned {:?} [{:?}]", &err, &data);
                        return data;
                    }
                }
                // Notify the condvar that the value has changed.
                cvar.notify_one();            
            } // drop the permit lock
            // Load the start state
            let state_opt = Data::start_state();
            // Anything non-zero is an  abort state, for now
            //let mut is_abort;
            while !(signal.load(std::sync::atomic::Ordering::Relaxed) != 0) {
                permits.acquire_permit();
                let mut state_res = MoveAgentStateMachine::advance_state_move(state_opt.clone(), &data);
                //is_abort = signal.load(std::sync::atomic::Ordering::Relaxed) != 0;
                // Use the outdated abort signal when the state
                while !state_res.is_none() {
                    match state_res {
                        Some(unwrapped) => {
                            if unwrapped.is_force_abort_state() {
                                break;
                            } else {
                                state_res = MoveAgentStateMachine::advance_state_move(unwrapped, &data);
                            }
                        },
                        None => {
                            break;
                        },
                    }
                }
                if signal.load(std::sync::atomic::Ordering::Relaxed) != 0 {
                    println!("Doing abort");
                    //signal.store(0, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
            }
            println!("Returning data {:?}",&data.clone());
            data
        });
        // Wait for the thread to start up.
        let (lock, cvar) = &*permit_condvar;
        // Store permit from called thread
        let permits;
        '_locked_block_permit: {
            let mut started = lock.lock().unwrap();
            while started.is_none() {
                started = cvar.wait(started).unwrap();
            }

            if let Some(s) = started.clone() {
                permits = s;
            } else {
                return Err(AgentProgramError::NoPermits);
            }
        }
        // Store data from called thread
        let (lock, cvar) = &*data_condvar;
        '_locked_block_data : {
            // Todo: maybe we want a timeout on the lock
            let mut started = lock.lock().unwrap();
            while started.is_none() {
                started = cvar.wait(started).unwrap();
            }
            if let Some(data_ret) = started.clone() {
                let aref = AgentRef::new(data_ret, permits);
                Ok(Self {
                    agent_thread: handle,
                    agent_ref: aref,
                    _phantom: PhantomData,
                    signal_atomic_algebra: abort_atomic,
                })
            } else {
                return Err(AgentProgramError::NoData);
            }
        }
    }
    pub fn finish(self)->Option<Arc<Data>>{
        match self.agent_thread.join() {
            Ok(data) => {Some(data)},
            Err(_) => {
                None
            },
        }
    }
    /// Indicate something has changed to poke the agent, then yield the current thread.
    pub fn poke(&self){
        self.agent_ref.state_changed();
    }

    // To mark a signal, 
    fn mark_signal(&self, signal: SignalSized)->SignalSized {
        AtomicSignalAlgebra::release_or(&self.signal_atomic_algebra, signal)
    }

    pub fn abort(&self) {
        // Mark the scheduler for abortion
        self.mark_signal(Signal::ABORT);
        // Poke the scheduler
        self.poke();
    }
}
// A unified structure
#[derive(Debug)]
pub struct CloneAgentProgram<Config, Data: Send + Sync + 'static> {
    agent: Arc<CloneAgent<Data>>,
    agent_ref: AgentRef<Data>,
    _phantom: PhantomData<Config>
}

impl<Config, Data: Send + Sync + 'static> Clone for CloneAgentProgram<Config, Data> {
    fn clone(&self) -> Self {
        Self { agent: self.agent.clone(), agent_ref: self.agent_ref.clone(), _phantom: self._phantom.clone() }
    }
}
#[derive(Debug)]
pub enum AgentProgramError<T> {
    NoRef, // No agent reference returned
    AgentRunError(AgentRunStatus),
    AgentRunning(T),
    AgentErrorState(String),
    NoPermits,
    NoData
}
impl<Config, Data: Send + Sync + 'static + FromConfigClone<Config>> CloneAgentProgram<Config, Data> {
    /// Create & run a new agent from config
    pub fn start(config: &Config)->Result<Self, AgentProgramError<Self>> {
        let start_state = Data::start_state(config);
        let mut agent = CloneAgent::new(start_state, Data::from_config(config));
        agent.run_clone();
        // IF the permit store and data are ready at the agent, we pass
        let agent_ref = agent.get_ref();
        match agent_ref {
            Some(agent_ref) => {
                Ok(CloneAgentProgram { agent: Arc::new(agent), agent_ref, _phantom: PhantomData })
            },
            None => Err(AgentProgramError::NoRef),
        }
    }
    /// Signal the agent to abort. Non-blocking
    pub fn abort(&self){
        self.agent.abort();
    }
    /// Abort the agent and wait for it to complete. Blocking abort-complete operation.
    pub fn stop(self)->Result<Arc<Data>, AgentProgramError<Self>> {
        self.agent.abort();
        self.complete()
    }

    /// Clone the agent reference object
    pub fn get_ref(&self)->AgentRef<Data>{
        self.agent_ref.clone()
    }
    /// Check the status of the agent's scheduler thread
    pub fn get_status(&self)->AgentThreadStatus{
        self.agent.get_thread_status()
    }

    /// Wait for the agent to complete, returning a reference to the agent data
    pub fn complete(self)->Result<Arc<Data>, AgentProgramError<Self>>{
        let agent;
        // If we can't move the arc, return owned value
        if Arc::strong_count(&self.agent) > 1 {
            return Err(AgentProgramError::AgentRunning(self));
        }

        //  DISCLAIMER: Since we own `self`, and there is a strong reference count of one, 
        //  then it should be impossible that another strong reference exists outside this function.
        //  However, it is possible that a weak reference could be upgraded in the time between the previous check, and the unwrap below
        //  Since we can't return `AgentProgramError::AgentRunning(self)` after doing `into_inner` because of a partial move
        //  we use a `Arc::try_unwrap` and the reconstruct the `AgentProgram` from the old agent reference
        
        // Move the inner agent out of the arc, if possible, or we return an agent running error.
        match Arc::try_unwrap(self.agent) {
            Ok(_agent) => agent = _agent,
            Err(old_arc) => {
                let s = String::from("Agent still has outstanding references, or is still running");
                println!("{}",&s);
                // This is a slower operation, but we should rarely (never if we have no weak references to the agent) get to this point since we do a `Arc::strong_count` on the agent arc
                let ap = CloneAgentProgram { agent: old_arc, agent_ref: self.agent_ref, _phantom: PhantomData::<Config> };
                return Err(AgentProgramError::AgentRunning(ap));
            },
        }

        ///todo: could run a store on the finish
        // Finish the agent and return the data reference, or an error.
        match agent.finish() {
            Ok(status) => {
                if status == AgentRunStatus::Success {
                    Ok(self.agent_ref.clone_data())
                } else {
                    Err(AgentProgramError::AgentRunError(status))
                }
            },
            Err(err) => {
                Err(AgentProgramError::AgentErrorState(err))
            },
        }
    }

    /// Indicate something has changed to poke the agent, then yield the current thread
    pub fn poke(&self){
        self.agent_ref.state_changed();
    }
}