use spacetimedb::{table, ScheduleAt};
use crate::reducers::lifecycle::expire_buffs;

#[table(name = buff_expiry_schedule, scheduled(expire_buffs))]
#[derive(Clone)]
pub struct BuffExpirySchedule {
    #[primary_key]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
}
