use std::sync::{Arc, RwLock};

use modular_agents::{agent_program::{AgentProgram, FromConfig}, state::State};

fn main(){
    struct SimpleAgentConfig<'a> {
        name: &'a str
    }
    struct SimpleAgentData {
        name: String,
        repeats: RwLock<usize>,
    }
    struct InitState;
    struct FinishState;
    impl State<SimpleAgentData> for FinishState{
        fn is_abort_state(&self)->bool {
            true
        }
    }
    impl State<SimpleAgentData> for InitState {
        fn pick_and_execute_an_action(self: Arc<Self>, data: &Arc<SimpleAgentData>)->Option<Arc<dyn State<SimpleAgentData>>> {

            let repeats;
            match data.repeats.read() {
                Ok(_reader) => {
                    repeats = *_reader;
                },
                Err(err) => {
                    println!("Reader is poisoned {}, reviving", err.to_string());
                    data.repeats.clear_poison();
                    return Some(self);
                },
            }              
            println!("[{}] Repeat count = {}", &data.name, repeats);
            match data.repeats.write(){
                Ok(mut writer) => {
                    *writer -= 1;
                },
                Err(err) => {
                    println!("Writer is poisoned {}, reviving", err.to_string());
                    data.repeats.clear_poison();
                    return Some(self);
                },
            }

            if repeats > 1 { //we did a decrement already (or panicked)
                Some(self)
            } else {
                Some(Arc::new(FinishState))
            }
        }
    }
    impl<'a> FromConfig<SimpleAgentConfig<'a>> for SimpleAgentData {
        fn from_config(c: &SimpleAgentConfig<'a>)->Arc<Self> {
            Arc::new(SimpleAgentData {name: c.name.to_string(), repeats: RwLock::new(5)})
        }
    
        fn start_state(_c: &SimpleAgentConfig<'a>)->Arc<dyn State<Self>> {
            Arc::new(InitState)
        }
    }
    
    let config1 = SimpleAgentConfig { name: "agent 1"};
    let ap = AgentProgram::<SimpleAgentConfig, SimpleAgentData>::start(&config1);
    assert!(ap.is_ok());
   
    match ap {
        Ok(agent_program) => {
            agent_program.poke();
            assert!(agent_program.complete().is_ok());
        },
        Err(err)=>{
            assert!(false, "Failed to get agent program {:?}",err);
        }
    }
}