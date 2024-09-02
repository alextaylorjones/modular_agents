use std::sync::Arc;

/// A trait which we can our scheduler `pick_and_execute_an_action`. Optionally, we can overwrite the `is_abort_state` to signal
/// that a state is a terminal state in the state machine
pub trait State<Data> 
    where Self: std::fmt::Debug + Send + Sync + 'static, Data: Sized + Send + Sync + 'static {
    /// Perform a scheduler operation:
    /// Requires access to a data object
    /// Returns None if the scheduler is finished with all work-in-progress
    /// If more work is to be done in some state S, then returns Ok(S)
    fn pick_and_execute_an_action(&self, _data: &Arc<Data>)->Option<Arc<dyn State<Data>>> {
        None
    }
    /// Allows the state to auto-trigger an abort sequence
    fn is_abort_state(&self)->bool {
        false
    }
}

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


/// Stores a representation of the current state
pub struct AgentStateMachine<T> {
    cur: Arc<dyn State<T>>
}


impl<T: Send + Sync + 'static> AgentStateMachine<T> {
    pub fn new(start: Arc<dyn State<T>>)->Self {
        AgentStateMachine { cur: start }
    }
    /// Run the state scheduler and advance the machine to the next state
    //pub fn advance_state(&mut self, data: &Arc<T>)->Option<Arc<dyn State<T>>> where Self : Sized {
    pub fn advance_state(&mut self, data: &Arc<T>)->bool where Self : Sized {
        
        let next_state = self.cur.pick_and_execute_an_action(data);
        match next_state {
            Some(next_state) => {
                //println!("More work still pending: {:?}->{:?}", &self.cur, &next_state);
                self.cur = next_state;
                //Some(next_state)
                true
            },
            None => {
                //println!("No new work to complete: {:?}->{:?}", &self.cur, &self.cur);
                //None
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