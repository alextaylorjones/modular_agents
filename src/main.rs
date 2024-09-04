use std::{rc::Rc, sync::{atomic::AtomicUsize, Arc}, time::Instant};

use modular_agents::{agent::{Agent, AgentRunStatus}, state::State};



fn main(){
    let now = Instant::now();
    const N: usize = 100;
    struct AgentData {
        my_stored_val: Arc<AtomicUsize>
    }

    #[derive(Debug)]
    struct MyAgentStartState;

    impl State<AgentData> for MyAgentStartState {
        fn pick_and_execute_an_action(self: Arc<Self>, data: &std::sync::Arc<AgentData>)->Option<std::sync::Arc<dyn State<AgentData>>> {
            if data.my_stored_val.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) > 0 {
                Some(self)
            } else {
                None
            }
        }
        fn is_abort_state(&self)->bool {
            true
        }
    }
    let start_state = Arc::new(MyAgentStartState);
    let mut agents = Vec::new();
    for i in 0..N {
        let a1_data = AgentData {my_stored_val: Arc::new(AtomicUsize::new(10000000))};
        let data_store = Arc::new(a1_data);
        let mut a1 = Agent::new(start_state.clone(), data_store);
        a1.run();
        agents.push(a1);
    }
    let then = Instant::now() - now;
    let now2 = Instant::now();
    let _ = agents.iter().map(|a1| {
        let _ = a1.state_changed();
    });
    let _ = agents.into_iter().map(|a1| {
        let _ = a1.finish();
    });
    let then2 = Instant::now() - now2;
    println!("Took {:?} to start, {:?} to run", then, then2);
}