use std::{sync::{Arc, Mutex}, thread, time::Duration};

use modular_agents::{agent::{Agent, AgentRunStatus}, state::State};

fn main(){
    struct AgentStringData {
        my_stored_val: Arc<Mutex<String>>,
    }

    #[derive(Debug)]
    struct WorkState;
    #[derive(Debug)]
    struct FinishState;

    impl State<AgentStringData> for WorkState {
        fn pick_and_execute_an_action(&self, _data: &Arc<AgentStringData>)->Option<Arc<dyn State<AgentStringData>>> {
            {
                let s_guard = _data.my_stored_val.lock();
                let mut s;
                match s_guard {
                    Ok(val) => {
                        s = val;
                    },
                    Err(err) => {
                        println!("Something was poisoned! {:?}", err);
                        return Some(Arc::new(FinishState)); 
                    },
                }
                println!("[{:?}]?Current string: {}",thread::current().id(),s);
                let n = s.len()-1;
                if n == 0 {
                    return Some(Arc::new(FinishState));
                }
                let _ = s.split_off(n);                    
                if s.len() > 0 {
                    Some(Arc::new(WorkState))
                }
                else {
                    Some(Arc::new(FinishState))
                }
            }
        }
    }

    impl State<AgentStringData> for FinishState {
        fn is_abort_state(&self)->bool {
            true
        }
    }
    let start_state = Arc::new(WorkState);
    let a1_data = AgentStringData {my_stored_val: Arc::new(Mutex::new(String::from("this is the string to process")))};
    let data_store = Arc::new(a1_data);
    let mut a1 = Agent::new(start_state, data_store);
    let mut a2 = a1.clone();
    
    a1.run();
    a2.run();
    [&a1,&a2].map(|a| assert!(a.has_valid_handle()));
    assert!(a1.state_changed());
    assert!(a2.state_changed());
    thread::sleep(Duration::from_millis(10));
    a1.abort();
    a2.abort();
    let _ = a1.finish().is_ok_and(|status| status == AgentRunStatus::Success);
    let _ = a2.finish().is_ok_and(|status| status == AgentRunStatus::Success);
}