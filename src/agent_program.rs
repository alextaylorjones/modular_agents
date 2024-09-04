use std::{marker::PhantomData, sync::Arc, thread};

use crate::{agent::{Agent, AgentRef, AgentRunStatus}, state::State};



// A unified structure
pub struct AgentProgram<Config, Data: Send + Sync + 'static> {
    agent: Arc<Agent<Data>>,
    agent_ref: AgentRef<Data>,
    _phantom: PhantomData<Config>
}
pub trait FromConfig<Config> {
    fn from_config(c: &Config)->Arc<Self>;
    fn start_state(c: &Config)->Arc<dyn State<Self>>;
}

#[derive(Debug)]
pub enum AgentProgramError {
    NoRef,
    AgentRunError(AgentRunStatus),
    AgentRunning,
    AgentErrorState(String)
}
impl<Config, Data: Send + Sync + 'static + FromConfig<Config>> AgentProgram<Config, Data> {
    /// Create & run a new agent from config
    pub fn start(config: &Config)->Result<Self, AgentProgramError> {
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
    /// Abort the agent and wait for it to complete
    pub fn stop(self)->Result<Arc<Data>, AgentProgramError> {
        self.agent.abort();
        self.complete()
    }

    /// Wait for the agent to complete
    pub fn complete(self)->Result<Arc<Data>, AgentProgramError>{
        let agent;
        // Move the agent out of the arc, if possible
        match Arc::into_inner(self.agent) {
            Some(_agent) => agent = _agent,
            None => {
                println!("Agent still has outstanding references, or is still running");
                return Err(AgentProgramError::AgentRunning);
            },
        }
        // Finish the agent
        match agent.finish() {
            Ok(status) => {
                if status == AgentRunStatus::Success {
                    Ok(self.agent_ref.data)
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
        self.agent.state_changed();
        thread::yield_now();
    }
}