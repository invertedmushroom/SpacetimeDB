use spacetimedb::{table, Timestamp};

#[table(name = contact_event, public)]
#[derive(Clone)]
pub struct ContactEvent {
    #[primary_key]
    pub id: u64,
    pub entity_1: u32,
    pub entity_2: u32,
    pub started_at: Timestamp,
}
