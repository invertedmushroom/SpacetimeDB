use spacetimedb::{table, Identity, Timestamp};

#[table(name = contact_event, public)]
#[derive(Clone)]
pub struct ContactEvent {
    #[primary_key]
    pub id: u64,
    pub option_id: Identity,
    pub entity_1: Identity,
    pub entity_2: Identity,
    pub region: u32,
    pub started_at: Timestamp,
}
