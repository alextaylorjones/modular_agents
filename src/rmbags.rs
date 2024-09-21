use std::{marker::{PhantomData, PhantomPinned}, sync};

// Goal: I want to have a generic Agent class that does has same behavior as crate::agent, but I want to separate the agent into four distinct classes


// State is all of the un-syncable agent data sources
// We build a state from a config type
pub trait MessageStore<Config, Store> where Config: Send + 'static, Store: MessageStoreConfig<Config> { }
pub trait MessageStoreConfig<Config>{ }

// Data is all of the sync-able agent data stores
pub trait DataPool<T> where T: Send + Sync + 'static { }
// Scheduler is 
pub trait Scheduler<T,U> { }
pub trait ActionSet {}

// Idea: write example code for a macro that turns the agent code into a state enum
// if we did this, we could store a last-known state variable
// and only create at most one state 
// For large numbers of states, could store "most recent" 


//  In the regular lib, states define the scheduler operation, and all data was consistent across states
//      eed
    

#[cfg(test)]
mod normative {
   use super::*;

    #[test]
    fn normative(){



        
    }   
}