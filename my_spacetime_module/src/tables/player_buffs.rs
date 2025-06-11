use spacetimedb::{ Timestamp, Identity};

#[spacetimedb::table(name = player_buffs, public)]
#[derive(Clone, Debug)]
pub struct PlayerBuff {
    /// surrogate PK so players can have multiple buffs
    #[primary_key]
    pub id: u64,

    /// filter by player
    #[index(btree)]
    pub player_id: Identity,
    
    pub stacks: u8,         // stacks or multiple entries in db?
    pub buff_type: u8,      // e.g. CD_REDUCTION
    pub magnitude: f32,     // e.g. 0.2 = 20% reduction
    #[index(btree)]
    pub expires_at: Timestamp,
}