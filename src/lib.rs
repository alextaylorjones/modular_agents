pub mod permit_store;
pub mod agent;
pub mod state;


fn _print_thread_info(){
    print!("<{:?}>",std::thread::current().id());
}

#[cfg(test)]
mod tests {
    use std::{sync::{atomic::AtomicUsize, Arc, Mutex}, thread, time::Duration};
    use agent::{Agent, AgentRunStatus};
    use state::State;
    
    use super::*;

    #[test]
    fn normative_one_agent_one_state() {
        struct AgentData {
            my_stored_val: usize
        }
    
        #[derive(Debug)]
        struct MyAgentStartState;
    
        impl State<AgentData> for MyAgentStartState {
            fn pick_and_execute_an_action(&self, data: &std::sync::Arc<AgentData>)->Option<std::sync::Arc<dyn State<AgentData>>> {
                _print_thread_info();
                println!("Running pick_and_execute_an_action for MyAgent: Data = {:?}", &data.my_stored_val);
                None
            }
        }
        let start_state = Arc::new(MyAgentStartState);
        let a1_data = AgentData {my_stored_val: 10};
        let data_store = Arc::new(a1_data);
        let mut a1 = Agent::new(start_state, data_store);
        
        a1.run();
        assert!(a1.state_changed());
        //thread::sleep(Duration::from_millis(10));
        a1.abort();
        assert!(a1.finish().is_ok_and(|status| status == AgentRunStatus::Success));
    }

    #[test]
    fn normative_one_agent_two_states(){
        struct AgentData {
            my_stored_val: AtomicUsize
        }
    
        #[derive(Debug)]
        struct MyAgentStartState;
        #[derive(Debug)]
        struct MyAgentFinishState;
        
        impl State<AgentData> for MyAgentFinishState {
            fn pick_and_execute_an_action(&self, data: &Arc<AgentData>)->Option<Arc<dyn State<AgentData>>> {
                _print_thread_info();
                println!("Running pick_and_execute_an_action on AgentData in MyAgentFinishState: Data = {:?}", &data.my_stored_val);
                let fs = Arc::new(MyAgentFinishState);
                if data.my_stored_val.load(std::sync::atomic::Ordering::Relaxed) > 0 {
                    data.my_stored_val.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    return Some(fs);
                }
                None
            }
        }
        impl State<AgentData> for MyAgentStartState {
            fn pick_and_execute_an_action(&self, data: &std::sync::Arc<AgentData>)->Option<std::sync::Arc<dyn State<AgentData>>> {
                _print_thread_info();
                println!("Running pick_and_execute_an_action on AgentData in MyAgentStartState: Data = {:?}", &data.my_stored_val);
                let fs = Arc::new(MyAgentFinishState);
                Some(fs)
            }
        }
        let start_state = Arc::new(MyAgentStartState);
        let a1_data = AgentData {my_stored_val: AtomicUsize::new(10)};
        let data_store = Arc::new(a1_data);
        let mut a1 = Agent::new(start_state, data_store);
        
        a1.run();
        assert!(a1.has_valid_handle());
        assert!(a1.state_changed());
        thread::sleep(Duration::from_millis(10));
        a1.abort();
        assert!(a1.finish().is_ok_and(|status| status == AgentRunStatus::Success));
    }

    #[test]
    fn normative_two_agents_same_data(){
        struct AgentData {
            my_stored_val: AtomicUsize
        }
    
        #[derive(Debug)]
        struct MyAgentStartState;
        #[derive(Debug)]
        struct MyAgentFinishState;
        
        impl State<AgentData> for MyAgentFinishState {
            fn pick_and_execute_an_action(&self, data: &Arc<AgentData>)->Option<Arc<dyn State<AgentData>>> {
                println!("<{:?}> Running pick_and_execute_an_action on AgentData in MyAgentFinishState: Data = {:?}", thread::current().id(), &data.my_stored_val);
                let fs = Arc::new(MyAgentFinishState);
                if data.my_stored_val.load(std::sync::atomic::Ordering::Relaxed) > 0 {
                    data.my_stored_val.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    return Some(fs);
                }
                None
            }
        }

        impl State<AgentData> for MyAgentStartState {
            fn pick_and_execute_an_action(&self, data: &std::sync::Arc<AgentData>)->Option<std::sync::Arc<dyn State<AgentData>>> {
                println!("<{:?}> Running pick_and_execute_an_action on AgentData in MyAgentStartState: Data = {:?}", thread::current().id(), &data.my_stored_val);
                let fs = Arc::new(MyAgentFinishState);
                Some(fs)
            }
        }

        let start_state = Arc::new(MyAgentStartState);
        let a1_data = AgentData {my_stored_val: AtomicUsize::new(30)};
        let data_store = Arc::new(a1_data);
        let mut a1 = Agent::new(start_state, data_store.clone());
        // Make another consumer of the same data with the same behavior
        let mut a2 = a1.clone();
        
        a1.run();
        a2.run();
        [&a1,&a2].map(|a| assert!(a.has_valid_handle()));
        assert!(a1.state_changed());
        assert!(a2.state_changed());
        thread::sleep(Duration::from_millis(10));
        a1.abort();
        a2.abort();
        assert!(a1.finish().is_ok_and(|status| status == AgentRunStatus::Success));
        assert!(a2.finish().is_ok_and(|status| status == AgentRunStatus::Success));
        assert!(data_store.my_stored_val.load(std::sync::atomic::Ordering::Relaxed)==0)        
    }
    #[test]
    fn normative_two_state_w_abort_state(){
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
                    let mut s = _data.my_stored_val.lock().unwrap();
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
        
        a1.run();
        assert!(a1.state_changed());
        assert!(a1.finish().is_ok_and(|status| status == AgentRunStatus::Success));
    }
    #[test]
    fn normative_ref_state(){
        // struct AgentStringData {
        //     my_stored_val: Arc<Mutex<String>>,
        // }
        // #[derive(Debug)]
        // struct WorkState;
        // #[derive(Debug)]
        // struct FinishState;
        // impl State<AgentStringData> for FinishState {
        //     fn pick_and_execute_an_action(&self, _data: &Arc<AgentStringData>)->Option<Arc<dyn State<AgentStringData>>> {
        //         None
        //     }
        // }
        // impl RefState<AgentStringData> for WorkState {
        //     fn pick_and_execute_an_action(&self, _data: &Arc<AgentStringData>)->&impl State<AgentStringData> {
        //         &FinishState
        //     }
        // }
    }
}