pub mod permit_store;
pub mod agent;
pub mod state;
pub mod message;
pub mod agent_program;


fn _print_thread_info(){
    print!("<{:?}>",std::thread::current().id());
}
pub fn thread_info()->String{
    return format!("<{:?}>",std::thread::current().id());
}

#[cfg(test)]
mod tests {
    use std::{sync::{atomic::{AtomicIsize, AtomicUsize}, Arc, Mutex, RwLock, Weak}, thread::{self}, time::Duration};
    use agent::{Agent, AgentRef, AgentRunStatus};
    use agent_program::{AgentProgram, FromConfig};
    use message::{MessageTarget, StoresSingle};
    
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
            fn pick_and_execute_an_action(self: Arc<Self>, data: &std::sync::Arc<AgentData>)->Option<std::sync::Arc<dyn State<AgentData>>> {
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
            fn pick_and_execute_an_action(self: Arc<Self>, data: &Arc<AgentData>)->Option<Arc<dyn State<AgentData>>> {
                _print_thread_info();
                println!("Running pick_and_execute_an_action on AgentData in MyAgentFinishState: Data = {:?}", &data.my_stored_val);
                if data.my_stored_val.load(std::sync::atomic::Ordering::Relaxed) > 0 {
                    data.my_stored_val.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    return Some(self);
                }
                None
            }
        }
        impl State<AgentData> for MyAgentStartState {
            fn pick_and_execute_an_action(self: Arc<Self>, data: &std::sync::Arc<AgentData>)->Option<std::sync::Arc<dyn State<AgentData>>> {
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
            fn pick_and_execute_an_action(self: Arc<Self>, data: &Arc<AgentData>)->Option<Arc<dyn State<AgentData>>> {
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
            fn pick_and_execute_an_action(self: Arc<MyAgentStartState>, data: &std::sync::Arc<AgentData>)->Option<std::sync::Arc<dyn State<AgentData>>> {
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
            fn pick_and_execute_an_action(self: Arc<WorkState>, _data: &Arc<AgentStringData>)->Option<Arc<dyn State<AgentStringData>>> {
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
            fn pick_and_execute_an_action(self: Arc<Self>, _data: &Arc<AgentStringData>)->Option<Arc<dyn State<AgentStringData>>> {
                None
            }
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

        struct DataSafeVec {
            stored_nums: Arc<Mutex<Vec<usize>>>,
            target_agent: Arc<Mutex<Weak<Agent<DataSafeVec>>>>,
        }

        impl StoresSingle<usize> for DataSafeVec {
            fn store(&self, data: usize) {
                if let Ok(mut guard) = self.stored_nums.lock(){
                    guard.push(data);
                } else {
                    // if another thread panicked while holding the lock, clear the poison and try again
                    self.stored_nums.clear_poison();
                    self.store(data);
                }
            }
        }

        impl StoresSingle<Arc<Agent<DataSafeVec>>> for DataSafeVec {
            fn store(&self, data: Arc<Agent<DataSafeVec>>) {
                if let Ok(mut guard) = self.target_agent.lock() {
                    let downgraded_data = Arc::downgrade(&data);
                    *guard = downgraded_data;
                }  else {
                    // if another thread panicked while holding the lock, clear the poison and try again
                    self.target_agent.clear_poison();
                    self.store(data);
                }
            }
        }

        impl MessageTarget<Arc<Agent<DataSafeVec>>> for Agent<DataSafeVec> {
            fn msg(self: Arc<Agent<DataSafeVec>>, data: Arc<Agent<DataSafeVec>>) {
                let data_store = self.get_data();
                data_store.store(data);
                self.state_changed();
            }
        }

        impl MessageTarget<usize> for Agent<DataSafeVec> {
            fn msg(self: Arc<Agent<DataSafeVec>>, data: usize) {
                // Get data store
                let data_store = self.get_data();
                // Add the new data
                data_store.store(data);
                // Signal a state change
                self.state_changed();
            }
        }

        let asd_1 = Arc::new(DataSafeVec { stored_nums: Arc::new(Mutex::new(vec![1,2,3])), target_agent: Arc::new(Mutex::new(Weak::new())) });
        let asd_2 = Arc::new(DataSafeVec { stored_nums: Arc::new(Mutex::new(vec![4,5,6])), target_agent: Arc::new(Mutex::new(Weak::new())) });
        
        struct InitState;
        struct DeliverState;
        struct FinishState;

        impl State<DataSafeVec> for DeliverState {
            fn pick_and_execute_an_action(self: Arc<Self>, data: &Arc<DataSafeVec>)->Option<Arc<dyn State<DataSafeVec>>> {
                println!("{} in deliver state, currently storing {:?}", thread_info(), &data.stored_nums.lock().unwrap());
                let num;
                match data.stored_nums.lock(){
                    Ok(mut nums_guard) => {
                        num = nums_guard.pop()
                    },
                    Err(_err) => {
                        return Some(Arc::new(FinishState));
                    },
                }

                if let Some(num) = num {
                    // If our num has been passed too many times, don't add it back or send it along
                    if num > 20 {
                        return Some(self);
                    }

                    if let Ok(guard) = data.target_agent.lock() {
                        match guard.upgrade() {
                            Some(target) => {
                                target.msg(num+1);
                                return Some(self);
                            },
                            None => {
                                println!("{:?}No connection to target agent, moving back to init state", thread_info());
                                // Add the number back into the data store 
                                match data.stored_nums.lock(){
                                    Ok(mut nums_guard) => {
                                        nums_guard.push(num)
                                    },
                                    Err(_err) => {
                                        return Some(Arc::new(FinishState));
                                    },
                                }
                                return Some(Arc::new(InitState));
                            },
                        }
                    } else {
                        println!("{}Target agent mutex has been poisoned, clearing poison and retrying", thread_info());
                        data.target_agent.clear_poison();
                        return Some(self);
                    }
                    
                } else { // Empty vector means we are done
                    println!("{} No more work to complete, moving to finish state",thread_info());
                    Some(Arc::new(FinishState))
                }
            }
        }
        impl State<DataSafeVec> for InitState {
            fn pick_and_execute_an_action(self: Arc<Self>, data: &Arc<DataSafeVec>)->Option<Arc<dyn State<DataSafeVec>>> {
                println!("{} in init state, currently storing {:?}", thread_info(), &data.stored_nums);
                // If we have a connection to the target, move to delivery state
                if let Ok(guard) = data.target_agent.lock() {
                    match guard.upgrade() {
                        Some(_) => {
                            // Here we can set the target in the delivery state so we are guaranteed to have the connection
                            return Some(Arc::new(DeliverState));
                        },
                        None => {
                            // Not connected: Wait for a new message
                            return None;
                        },
                    }
                } else {
                    println!("{}Target agent is poisoned", thread_info());
                    data.target_agent.clear_poison();
                    return Some(self);
                }

            }
        }
        impl State<DataSafeVec> for FinishState {
            fn pick_and_execute_an_action(self: Arc<Self>, data: &Arc<DataSafeVec>)->Option<Arc<dyn State<DataSafeVec>>> {
                println!("{} in finish state", thread_info());
                match data.stored_nums.lock() {
                    Ok(data) => {
                        println!("{} Successfully got to data in finished state: {:?}", thread_info(),data);
                    },
                    Err(err) => {
                        println!("<{:?}>Failed to get data in finished state: {:?}", thread_info(),err);
                    },
                }
                // Always exit
                None
            }
            fn is_abort_state(&self)->bool { true }
        }

        let mut ca_1 = Agent::new(Arc::new(InitState), asd_1.clone());
        let mut ca_2 = Agent::new(Arc::new(InitState), asd_2.clone());
        ca_1.run();
        ca_2.run();
        
        let ca_1_arc = Arc::new(ca_1);
        let ca_2_arc = Arc::new(ca_2);
        // Just let agent 1 connect to agent 2
        ca_1_arc.clone().msg(ca_2_arc.clone());
        ca_2_arc.clone().msg(ca_1_arc.clone());
        // match Arc::into_inner(ca_1_arc) {
        //     Some(agent) => {
        //         assert!(agent.finish().is_ok_and(|x| x== AgentRunStatus::Success));
        //     },
        //     None => {
        //         assert!(false, "Failed to move agent out of the arc");
        //         return;
        //     },
        // }
        
        thread::sleep(Duration::from_millis(10));
        //  while Arc::strong_count(&ca_1_arc) > 1 || Arc::strong_count(&ca_2_arc) > 1 {
        //     println!("Outstanding strong references to ca1 {}", Arc::strong_count(&ca_1_arc));
        //     println!("Outstanding strong references to ca2 {}", Arc::strong_count(&ca_2_arc));
        //     thread::sleep(Duration::from_millis(10));
        //  }
        
        for ag_arc in [ca_1_arc, ca_2_arc] {
            match Arc::into_inner(ag_arc) {
                Some(agent) => {
                    assert!(agent.finish().is_ok_and(|x| x== AgentRunStatus::Success));
                },
                None => {
                    assert!(false, "Failed to move agent out of the arc");
                },
            }
        }

        
         
        // assert!(ca_1_arc.has_valid_handle());
        // assert!(ca_2_arc.has_valid_handle());
        
        // let asd_1_clone = asd_1.clone();
        // let asd_2_clone = asd_2.clone();
        // let mut nums = Vec::new();
        // for n in asd_1_clone.stored_nums.lock().unwrap().iter() {
        //     nums.push(*n);
        // }
        // for n in asd_2_clone.stored_nums.lock().unwrap().iter() {
        //     nums.push(*n);
        // }
        // nums.sort();
        // assert_eq!(nums, vec![1,2,3,4,5,6]);

    }

    #[test]
    fn normative_three_agent(){
        // type ReadToCapacityAgent = Agent<StorageAgentData>;
        // type SendUntilEmptyAgent = Agent<SenderAgentData>;
        
        // type ReadTarget = Arc<Mutex<Arc<ReadToCapacityAgent>>>;
        
        // struct SenderAgentData {
        //     msg_targets: Vec<ReadTarget>
        // }

        // struct StorageAgentData {
        //     capacity: usize,
        //     stored_vals: Arc<RwLock<Vec<usize>>>
        // }

        // struct InitState;
        // struct ReadState;
        // struct SendState {
        //     start_idx: RwLock<usize>
        // }
        // struct FinishState;

        // // Both agents share a default finish state
        // impl<T: Send + Sync> State<T> for FinishState {
        //     fn is_abort_state(&self)->bool { true }
        // }
        // impl StoresVec<usize> for StorageAgentData {
        //     fn store_slice(&self, data: &[usize]) {
        //         match self.stored_vals.write() {
        //             Ok(mut write_guard) => {
        //                 write_guard.extend(data);
        //             },
        //             Err(err) => {
        //                 self.stored_vals.clear_poison();
        //                 self.store_slice(data);
        //             },
        //         }
        //     }
        // }
        // impl MessageTarget<Vec<usize>> for ReadToCapacityAgent {
        //     fn msg(self: Arc<Self>, data: Vec<usize>) {
        //         let data_store = self.get_data();
        //         data_store.store_slice(&data);
        //         self.state_changed();
        //     }
        // }
        // impl State<SenderAgentData> for InitState {
        //     fn pick_and_execute_an_action(self: Arc<Self>, data: &Arc<SenderAgentData>)->Option<Arc<dyn State<SenderAgentData>>> {
        //         if data.msg_targets.len() == 0 {
        //             return Some(Arc::new(SendState { start_idx: RwLock::new(0)}));
        //         } else {
        //             return Some(Arc::new(FinishState));
        //         }
        //     }
        // }
        // impl State<SenderAgentData> for SendState {
        //     fn pick_and_execute_an_action(self: Arc<Self>, data: &Arc<SenderAgentData>)->Option<Arc<dyn State<SenderAgentData>>> {
        //         let send_idx = *self.start_idx.read().unwrap() % data.msg_targets.len();
        //         *self.start_idx.write().unwrap() += 1;
        //         match data.msg_targets.get(send_idx) {
        //             Some(target) => {
        //                 let guard = target.lock().unwrap().clone();
        //                 //let reader: Arc<Agent<StorageAgentData>> = guard;
        //                 let send_data = vec![1,2,3];
        //                 guard.msg(send_data);
        //             },
        //             None => {
        //                 println!("{}No message targets, finishing", thread_info());
        //             },
        //         }
        //         None
        //     }
        // }
        // impl State<StorageAgentData> for ReadState {
        //     fn pick_and_execute_an_action(self: Arc<Self>, data: &Arc<StorageAgentData>)->Option<Arc<dyn State<StorageAgentData>>> {
        //         match data.stored_vals.read() {
        //             Ok(vals) => {
        //                 if vals.len() >= data.capacity {
        //                     println!("{} Capacity reached", thread_info());
        //                     return Some(Arc::new(FinishState));
        //                 } else {
        //                     return Some(self);
        //                 }
        //             },
        //             Err(err) => {
        //                 println!("Got a poisoned data store error={}, fixing and retrying", err.to_string());
        //                 data.stored_vals.clear_poison();
        //                 return Some(self);
        //             },
        //         }
        //     }
        // }

        // let reader_1_ds = Arc::new(StorageAgentData { capacity: 10, stored_vals: Arc::new(RwLock::new(Vec::new())) });
        // let reader_2_ds = Arc::new(StorageAgentData { capacity: 20, stored_vals: Arc::new(RwLock::new(Vec::new())) });
        
        // let reader_1 = ReadToCapacityAgent::new(Arc::new(ReadState), reader_1_ds);
        // let reader_2 = ReadToCapacityAgent::new(Arc::new(ReadState), reader_2_ds);
        
        // let mut targets = Vec::new();

        // let writer_ds = Arc::new(SenderAgentData { msg_targets: });
        // let writer = SendUntilEmptyAgent::new(Arc::new(InitState), writer_ds);
    }
    #[test]
    fn normative_one_writer_two_readers(){
        type SafeRwVec<T> = Arc<RwLock<Vec<T>>>;

        trait SafeAccess<T> {
            fn pop_panics(&self)->Option<T>;
            fn len(&self)->usize; 
            fn empty()->Self;
        }
        impl<T> SafeAccess<T> for SafeRwVec<T> {
            fn pop_panics(&self)->Option<T> {
                self.write().unwrap().pop()
            }
            fn empty()->Self {
                Arc::new(RwLock::new(Vec::new()))
            }
            
            fn len(&self)->usize {
                self.read().unwrap().len()
            }
        }

        //type AgentRef<T> = Arc<RwLock<T>>;
        type InputAgent = Agent<InputAgentDS>;
        type ReaderAgent = Agent<ReaderAgentDS>;

        // Define agents
        struct InputAgentDS {
            input_queue: SafeRwVec<usize>,
            output_target: AgentRef<ReaderAgentDS>
        }
        struct ReaderAgentDS {
            read_queue: SafeRwVec<usize>,
            last_queue_size: Arc<AtomicIsize>
        }
        impl ReaderAgentDS {
            pub fn new()->Self {
                let read_queue = SafeRwVec::empty();
                let last_queue_size = Arc::new(AtomicIsize::new(-1));
                ReaderAgentDS { read_queue, last_queue_size }
            }
        }
        struct InputWorkState;
        impl State<InputAgentDS> for InputWorkState {
            fn pick_and_execute_an_action(self: Arc<Self>, data: &Arc<InputAgentDS>)->Option<Arc<dyn State<InputAgentDS>>> {
                let next = data.input_queue.pop_panics();
                if let Some(val) = next {
                    println!("{} Got a value to send {}", thread_info(), val); 
                    data.output_target.msg_add_to_queue(val);
                    Some(self)
                } else {
                    None
                }
            }
        }

        struct ReaderWorkState;
        impl State<ReaderAgentDS> for ReaderWorkState {
            fn pick_and_execute_an_action(self: Arc<Self>, data: &Arc<ReaderAgentDS>)->Option<Arc<dyn State<ReaderAgentDS>>> {
                let last_size = data.last_queue_size.load(std::sync::atomic::Ordering::Relaxed);
                let cur_size = data.read_queue.len() as isize;
                if last_size > cur_size {
                    println!("Queue shrank from {} to {}", last_size, cur_size);
                } else if last_size < cur_size{
                    println!("Queue grew from {} to {}", last_size, cur_size);
                } else {
                    println!("Queue stayed the same at {} ", cur_size);
                }
                data.last_queue_size.store(cur_size as isize, std::sync::atomic::Ordering::Relaxed);
                let next = data.read_queue.pop_panics();
                if let Some(val) = next { 
                    println!("{} Consumed value in reader {}", thread_info(), val); 
                    Some(self)
                } else {
                    None
                }
            }
        }

        impl AgentRef<ReaderAgentDS> {
            pub fn msg_add_to_queue(&self, u: usize){
                match self.data.read_queue.write() {
                    Ok(mut q) => {
                        q.push(u);
                        
                        self.permit.release_permit();
                        
                        return;
                    },
                    Err(_) => {
                        self.data.read_queue.clear_poison();
                        self.msg_add_to_queue(u);  
                    },
                }                              
            }
        }

        impl ReaderAgent {
            pub fn create()->Self {
                ReaderAgent::new(Arc::new(ReaderWorkState), Arc::new(ReaderAgentDS::new()))
            }
        }
        impl InputAgent {
            pub fn msg_add_to_queue(&self, u: usize){
                let data = self.get_data();
                data.input_queue.write().unwrap().push(u);
                self.state_changed();
            }
            pub fn create(reader: AgentRef<ReaderAgentDS>)->Self {
                let store = InputAgentDS {
                    input_queue: SafeRwVec::empty(),
                    output_target: reader,
                };
                InputAgent::new(Arc::new(InputWorkState), Arc::new(store))
            }
        }
        
        let mut ra1 = ReaderAgent::create();
        ra1.run();
        let reader_data = ra1.get_data();
        let ra1_ref_opt = ra1.get_ref();
        let ra1_ref;
        match ra1_ref_opt {
            Some(_ra1_ref) => {
                ra1_ref = _ra1_ref;
            },
            None => {
                assert!(false, "No reference can be generated from the reader agent");
                return;
            },
        }
        let mut ia1 = InputAgent::create(ra1_ref.clone());
        let mut ia2 = InputAgent::create(ra1_ref.clone());
        // Run the agents
        ia1.run();
        ia2.run();

        for i in 0..10{
            if i % 2==0 {
                ia1.msg_add_to_queue(i);
            } else {
                ia2.msg_add_to_queue(i);
            }
        }

        thread::sleep(Duration::from_millis(10));

        ia1.abort();
        ia2.abort();
        ra1.abort();
        assert!(ia1.finish().is_ok_and(|x| x== AgentRunStatus::Success));
        assert!(ia2.finish().is_ok_and(|x| x== AgentRunStatus::Success));
        assert!(ra1.finish().is_ok_and(|x| x== AgentRunStatus::Success));
   
        assert!(reader_data.read_queue.read().unwrap().len() == 0);
    }

    #[test]
    fn normative_agent_program(){
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
                match agent_program.complete() {
                    Ok(result_data) => {
                        assert!(*result_data.repeats.read().unwrap() == 0);
                    },
                    Err(err) => {
                        assert!(false, "Agent program failed to return data as expected. {:?}",err);
                    },
                }
            },
            Err(err)=>{
                assert!(false, "Failed to get agent program {:?}",err);
            }
        }
    }
}