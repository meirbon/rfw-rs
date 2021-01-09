use crate::resources::ResourceList;
use rayon::prelude::*;

pub trait Plugin {
    fn init(&mut self, resources: &mut ResourceList, scheduler: &mut Scheduler);
}

pub trait System: 'static + Send + Sync {
    fn run(&mut self, resources: &ResourceList);
}

pub struct Scheduler {
    systems: Vec<Box<dyn System>>,
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler")
            .field("systems", &self.systems.len())
            .finish()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self {
            systems: Default::default(),
        }
    }
}

#[allow(dead_code)]
impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_system<S: 'static + System>(&mut self, system: S) {
        self.systems.push(Box::new(system));
    }

    pub fn run(&mut self, resources: &ResourceList) {
        self.systems.par_iter_mut().for_each(|s| s.run(resources));
    }
}
