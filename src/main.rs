use std::{rc::Rc, sync::{atomic::AtomicIsize, Arc}, thread, time::{Duration, Instant}};

use modular_agents::{agent::{Agent, AgentRunStatus}, state::State};
use par_map::ParMap;

fn benchmark()->Duration{
    let now = Instant::now();
    const N: usize = 10;
    const M: isize = 40000;
    struct AgentData {
        my_stored_val: Arc<AtomicIsize>
    }

    #[derive(Debug)]
    struct MyAgentStartState {
        count: AtomicIsize
    }

    impl State<AgentData> for MyAgentStartState {
        fn pick_and_execute_an_action(self: Arc<Self>, data: &std::sync::Arc<AgentData>)->Option<std::sync::Arc<dyn State<AgentData>>> {
            self.count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if data.my_stored_val.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) > 0 {
                Some(self)
            } else {
                Some(Arc::new(FinishState))
            }
        }

    }
    struct FinishState;
    impl State<AgentData> for FinishState {
        fn pick_and_execute_an_action(self: Arc<Self>, _data: &Arc<AgentData>)->Option<Arc<dyn State<AgentData>>> {
           // print!(".");
            None
        }
        fn is_abort_state(&self)->bool {
            true
        }
    }
    let start_state = Arc::new(MyAgentStartState {count: AtomicIsize::new(0)});
    let mut agents = Vec::new();
    let shared_val = Arc::new(AtomicIsize::new(M));
    for i in 0..N {
        let a1_data = AgentData {my_stored_val: shared_val.clone()};
        let data_store = Arc::new(a1_data);
        let mut a1 = Agent::new(start_state.clone(), data_store);
        a1.run();
        a1.state_changed();
        agents.push(a1);
    }
    let then = Instant::now() - now;
    let now2 = Instant::now();
    use par_map::Map;
    let _ = agents.into_iter().par_map(move |a| {
        let _ = a.finish();

    });

    let then2 = Instant::now() - now2;
    //println!("Took {:?} to start, {:?} to run", then, then2);
    return then+then2;
}

fn main(){
    const NUM_RUNS: usize = 100;
    let mut times = Vec::new();
    for _ in 0..NUM_RUNS {
        times.push(benchmark());
    }
    println!("Times (avg) {:?}, \n totals {:?}", times.iter().fold(Duration::ZERO,|a,x| a+*x) / NUM_RUNS as u32 , &times);
}