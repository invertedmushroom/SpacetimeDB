use spacetimedb::{Identity, Timestamp};

#[derive(Clone, Debug)]
#[spacetimedb::table(name = damage_event, public)]
pub struct DamageEvent {
    #[primary_key]
    #[auto_inc]
    pub event_id: u64,

    #[index(btree)]
    pub source_id: Identity,
    #[index(btree)]
    pub target_id: Identity,
    
    pub skill_id: u8,
    pub amount: u32,
    pub expire_at: Timestamp,
    pub region: u32,
}
