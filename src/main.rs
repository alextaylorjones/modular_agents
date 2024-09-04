use std::sync::{Arc, RwLock};

use modular_agents::{agent::AgentRef, agent_program::{AgentProgram, FromConfig}, state::State};

fn main(){
    let data = "test";
    let a = Arc::new(data);
    
    match Arc::into_inner(a.clone()) {
        Some(_) => {
            println!("Did it!");
        },
        None => {
            println!("CAnot do it");
        },
    }
}