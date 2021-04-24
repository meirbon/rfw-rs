use crate::schedule::RunOnce;
use crate::{ecs::schedule::SystemDescriptor, Instance};
pub use bevy_ecs::{prelude::*, *};
pub use bevy_tasks::ComputeTaskPool;

pub trait Bundle {
    fn init(self, instance: &mut Instance);
}

pub trait Plugin: Send + Sync {
    fn init(&mut self, instance: &mut crate::Instance);
}

pub struct Scheduler {
    schedule: Schedule,
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler").finish()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        let mut schedule: Schedule = Default::default();
        schedule.add_stage(
            CoreStage::Startup,
            Schedule::default()
                .with_run_criteria(RunOnce::default())
                .with_stage(StartupStage::PreStartup, SystemStage::parallel())
                .with_stage(StartupStage::Startup, SystemStage::parallel())
                .with_stage(StartupStage::PostStartup, SystemStage::parallel()),
        );
        schedule.add_stage(CoreStage::PreUpdate, SystemStage::parallel());
        schedule.add_stage(CoreStage::Update, SystemStage::parallel());
        schedule.add_stage(CoreStage::PostUpdate, SystemStage::parallel());

        Self { schedule }
    }
}

/// The names of the default App startup stages
#[derive(Debug, Hash, PartialEq, Eq, Clone, StageLabel)]
pub enum StartupStage {
    /// Name of app stage that runs once before the startup stage
    PreStartup,
    /// Name of app stage that runs once when an app starts up
    Startup,
    /// Name of app stage that runs once after the startup stage
    PostStartup,
}

/// The names of the default App stages
#[derive(Debug, Hash, PartialEq, Eq, Clone, StageLabel)]
pub enum CoreStage {
    /// Runs once at the beginning of the app.
    Startup,
    /// Name of app stage that runs before all other app stages
    /// Name of app stage responsible for performing setup before an update. Runs before UPDATE.
    PreUpdate,
    /// Name of app stage responsible for doing most app logic. Systems should be registered here
    /// by default.
    Update,
    /// Name of app stage responsible for processing the results of UPDATE. Runs after UPDATE.
    PostUpdate,
}

#[allow(dead_code)]
impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_startup_system(
        &mut self,
        stage_label: impl StageLabel,
        system: impl Into<SystemDescriptor>,
    ) {
        self.schedule
            .stage(CoreStage::Startup, |schedule: &mut Schedule| {
                schedule.add_system_to_stage(stage_label, system)
            });
    }

    pub fn add_system(
        &mut self,
        stage_label: impl StageLabel,
        system: impl Into<SystemDescriptor>,
    ) {
        self.schedule.add_system_to_stage(stage_label, system);
    }

    pub fn add_system_set(&mut self, stage_label: impl StageLabel, system: SystemSet) {
        self.schedule.add_system_set_to_stage(stage_label, system);
    }

    pub fn run(&mut self, resources: &mut World) {
        self.schedule.run_once(resources);
    }
}
