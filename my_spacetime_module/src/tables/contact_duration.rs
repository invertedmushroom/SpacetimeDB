use spacetimedb::{table, Timestamp, Identity};

#[table(name = contact_duration, public)]
#[derive(Clone)]
pub struct ContactDuration {
    #[primary_key]
    pub id: u64,                // auto-incrementing ID
    pub entity_1: Identity,
    pub entity_2: Identity,
    pub region: u32,
    pub started_at: Timestamp,
    pub duration_micros: i64,   // Duration in microseconds
}