use spacetimedb::{table, ScheduleAt};
use crate::physics::physics_tick::physics_tick;

#[table(name = physics_tick_schedule, scheduled(physics_tick))]
#[derive(Clone)]
pub struct PhysicsTickSchedule {
    #[primary_key]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
    pub region: u32,
}