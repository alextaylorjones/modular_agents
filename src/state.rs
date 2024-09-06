use std::{cell::RefCell, path::Iter, rc::Rc, sync::Arc, thread};

/// A trait which we can our scheduler `pick_and_execute_an_action`. Optionally, we can overwrite the `is_abort_state` to signal
/// that a state is a terminal state in the state machine. States should be lightweight and memory-less (leaving a state invalidates all state-specific memory)
/// 
/// There are no required implementations, 
pub trait State<Data> 
    where Self: Send + Sync + std::fmt::Debug, Data: Send + Sync {
    /// Perform a scheduler operation:
    /// Requires access to a data object
    /// Returns None if the scheduler is finished with all work-in-progress
    /// If more work is to be done in some state S, then returns Ok(S)
    fn pick_and_execute_an_action(self: Arc<Self>, _data: &Arc<Data>)->Option<Arc<dyn State<Data>>> {
        None
    }
    fn pick_and_execute_an_action_move(self: &mut Self, _data: &Arc<Data>)->Option<Rc<RefCell<dyn State<Data>>>> {
        None
    }
    /// Allows the state to auto-trigger an abort sequence
    fn is_abort_state(&self)->bool {
        false
    }
}

pub trait StateChanged {
    fn state_changed(&self);
}


// pub struct DoNothingAbort;
// impl<T: Send + Sync + 'static > State<T> for DoNothingAbort {
//     fn is_abort_state(&self)->bool {
//         true
//     }
    
//     fn pick_and_execute_an_action(self: Arc<Self>, _data: &Arc<T>)->Option<Arc<dyn State<T>>> {
//         None
//     }
// }

// pub trait RefState<Data>
//     where Self: std::fmt::Debug + Send + Sync + 'static, Data: Sized + Send + Sync + 'static {
//     /// Perform a scheduler operation:
//     /// Requires access to a data object
//     /// Returns None if the scheduler is finished with all work-in-progress
//     /// If more work is to be done in some state S, then returns Ok(S)
//     fn pick_and_execute_an_action(&self, _data: &Arc<Data>)->&impl State<Data>;
//     /// Allows the state to auto-trigger an abort sequence
//     fn is_abort_state(&self)->bool {
//         false
//     }
// }

pub type SchedulerReturn<T> = Option<Arc<dyn State<T>>>;
/// Stores a representation of the current state and controls how the state evolves by clone-on
pub struct AgentStateMachine<T> {
    cur: Arc<dyn State<T>>
}

pub struct MoveAgentStateMachine<T> {
    cur: Arc<dyn State<T>>,
    history: Vec<Rc<dyn State<T>>>,
}

// pub trait MovableState<T> where T: Send + Sync, Self: State<T> {
//     fn pick_and_execute_an_action_move
// }

impl<T: Send + Sync + 'static> MoveAgentStateMachine<T> {
    pub fn new(cur: Arc<dyn State<T>>)->Self {
        MoveAgentStateMachine { cur, history: Vec::new() }
    }       
    pub fn advance_state_move(state: Rc<RefCell<dyn State<T>>>, _data: &Arc<T>)->Option<Rc<RefCell<dyn State<T>>>> {
        let state = state.try_borrow_mut();
        match state {
            Ok(mut _unwrapped) => {
                 match _unwrapped.pick_and_execute_an_action_move(_data) {
                    Some(state_rc) => {
                        return Some(state_rc);
                    },
                    None => return None,
                }
            },
            Err(_err) =>  {
                println!("Borring mut error in moveagentstate");
                return None;
            }
        }
    }
    
}

// pub struct Record<T> {
//     prev_record: Rc<dyn State<T>>,
//     state_record: Rc<dyn State<T>>
// }
// impl<T> Iterator for MoveAgentStateMachine<T> {
//     type Item = Record<T>;
//     fn next(&mut self) -> Option<Self::Item> {
//     }
// }

impl<T: Send + Sync + 'static> AgentStateMachine<T> {
    pub fn new(start: Arc<dyn State<T>>)->Self {
        AgentStateMachine { cur: start }
    }
    // pub fn advance_state_move(state: Arc<dyn State<T>>, data: &Arc<T>)->Result<Arc<dyn State<T>>,Arc<dyn State<T>>> {
    //     let next_state = state.pick_and_execute_an_action_move(&data);
    //     match next_state {
    //         Some(next_state) => {
    //             //println!("More work still pending: {:?}->{:?}", &self.cur, &next_state);
    //             Ok(next_state)                
    //         },
    //         None => {
    //             //println!("No new work to complete: {:?}->{:?}", &self.cur, &self.cur);
    //             println!("[AgentStateMachine Thread # {:?}] None State: Strong refs to data {}, Weak refs to data {} ",thread::current(), Arc::strong_count(&data), Arc::weak_count(&data));
    //             Ok(state)
    //         },
    //     }
    // }
    /// Run the state scheduler and advance the machine to the next state
    pub fn advance_state_clone(&mut self, data: &Arc<T>)->bool  {
        // We call the scheduler on a clone of the current state
        let next_state = self.cur.clone().pick_and_execute_an_action(data);
        match next_state {
            Some(next_state) => {
                //println!("More work still pending: {:?}->{:?}", &self.cur, &next_state);
                self.cur = next_state;
                true
            },
            None => {
                //println!("No new work to complete: {:?}->{:?}", &self.cur, &self.cur);
                false
            },
        }
    }
    pub fn is_abort_state(&self)->bool{
        self.cur.is_abort_state()
    }
}

#[cfg(test)]
mod test{

    #[test]
    fn basictest(){
    }
}