//! Mock host: callbacks arrive from other threads after a configurable delay,
//! like UAPMD finishing work on its main thread.
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::Duration,
};

use crate::host::{CreateDone, Host, InstanceHandle, LoadDone, PluginKind};

#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    Create(String),
    Destroy(InstanceHandle),
    Load(InstanceHandle),
    Attach(u8, InstanceHandle),
    Detach(u8),
}

#[derive(Default)]
struct State {
    next: InstanceHandle,
    calls: Vec<Call>,
    alive: HashMap<InstanceHandle, String>,
    attached: HashMap<u8, InstanceHandle>,
}

#[derive(Default)]
pub struct MockHost {
    state: Mutex<State>,
    pub create_delay: Mutex<Duration>,
    pub load_delay: Mutex<Duration>,
    pub fail_create: Mutex<HashSet<String>>,
    pub fail_load: Mutex<bool>,
}

pub fn kind(plugin_id: &str) -> PluginKind {
    PluginKind {
        format: "CLAP".into(),
        plugin_id: plugin_id.into(),
    }
}

impl MockHost {
    pub fn calls(&self) -> Vec<Call> {
        self.state.lock().unwrap().calls.clone()
    }

    pub fn alive(&self) -> usize {
        self.state.lock().unwrap().alive.len()
    }

    pub fn attached(&self, slot: u8) -> Option<InstanceHandle> {
        self.state.lock().unwrap().attached.get(&slot).copied()
    }

    pub fn creates(&self) -> usize {
        self.count(|call| matches!(call, Call::Create(_)))
    }

    pub fn count(&self, matches: impl Fn(&Call) -> bool) -> usize {
        self.calls().iter().filter(|call| matches(call)).count()
    }

    fn record(&self, call: Call) {
        self.state.lock().unwrap().calls.push(call);
    }
}

impl Host for MockHost {
    fn create_instance(&self, kind: &PluginKind, done: CreateDone) {
        self.record(Call::Create(kind.plugin_id.clone()));
        let delay = *self.create_delay.lock().unwrap();
        let result = if self.fail_create.lock().unwrap().contains(&kind.plugin_id) {
            Err(format!("cannot create {}", kind.plugin_id))
        } else {
            let mut state = self.state.lock().unwrap();
            state.next += 1;
            let handle = state.next;
            state.alive.insert(handle, kind.plugin_id.clone());
            Ok(handle)
        };
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            done(result);
        });
    }

    fn destroy_instance(&self, handle: InstanceHandle) {
        self.record(Call::Destroy(handle));
        self.state.lock().unwrap().alive.remove(&handle);
    }

    fn load_state(&self, handle: InstanceHandle, _data: &[u8], done: LoadDone) {
        self.record(Call::Load(handle));
        let delay = *self.load_delay.lock().unwrap();
        let result = if *self.fail_load.lock().unwrap() {
            Err("bad state".to_string())
        } else {
            Ok(())
        };
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            done(result);
        });
    }

    fn attach_to_slot(&self, slot: u8, handle: InstanceHandle) -> bool {
        self.record(Call::Attach(slot, handle));
        self.state.lock().unwrap().attached.insert(slot, handle);
        true
    }

    fn detach_from_slot(&self, slot: u8) -> Option<InstanceHandle> {
        self.record(Call::Detach(slot));
        self.state.lock().unwrap().attached.remove(&slot)
    }
}
