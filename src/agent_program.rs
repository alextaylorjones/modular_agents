use std::{marker::PhantomData, sync::Arc};

use crate::{agent::{Agent, AgentRef, AgentRunStatus, AgentThreadStatus}, state::{State, StateChanged}};




// A unified structure
#[derive(Debug)]
pub struct AgentProgram<Config, Data: Send + Sync + 'static> {
    agent: Arc<Agent<Data>>,
    agent_ref: AgentRef<Data>,
    _phantom: PhantomData<Config>
}
impl<Config, Data: Send + Sync + 'static> Clone for AgentProgram<Config, Data> {
    fn clone(&self) -> Self {
        Self { agent: self.agent.clone(), agent_ref: self.agent_ref.clone(), _phantom: self._phantom.clone() }
    }
}
pub trait FromConfig<Config> {
    fn from_config(c: &Config)->Arc<Self>;
    fn start_state(c: &Config)->Arc<dyn State<Self>>;
}

#[derive(Debug)]
pub enum AgentProgramError<T> {
    NoRef,
    AgentRunError(AgentRunStatus),
    AgentRunning(T),
    AgentErrorState(String)
}
impl<Config, Data: Send + Sync + 'static + FromConfig<Config>> AgentProgram<Config, Data> {
    /// Create & run a new agent from config
    pub fn start(config: &Config)->Result<Self, AgentProgramError<Self>> {
        let start_state = Data::start_state(config);
        let mut agent = Agent::new(start_state, Data::from_config(config));
        agent.run();
        let agent_ref = agent.get_ref();
        match agent_ref {
            Some(agent_ref) => {
                Ok(AgentProgram { agent: Arc::new(agent), agent_ref, _phantom: PhantomData })
            },
            None => Err(AgentProgramError::NoRef),
        }
    }
    /// Signal the agent to abort. Non-blocking
    pub fn abort(&self){
        self.agent.abort();
    }
    /// Abort the agent and wait for it to complete. Blocking
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
        
        // Move the agent out of the arc, if possible
        match Arc::try_unwrap(self.agent) {
            Ok(_agent) => agent = _agent,
            Err(old_arc) => {
                let s = String::from("Agent still has outstanding references, or is still running");
                println!("{}",&s);
                // This is a slower operation, but we should rarely (never if we have no weak references to the agent) get to this point since we do a `Arc::strong_count` on the agent arc
                let ap = AgentProgram { agent: old_arc, agent_ref: self.agent_ref, _phantom: PhantomData::<Config> };
                return Err(AgentProgramError::AgentRunning(ap));
            },
        }

        // Finish the agent
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