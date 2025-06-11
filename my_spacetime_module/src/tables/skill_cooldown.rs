use spacetimedb::{ Timestamp, Identity};

#[derive(Clone, Debug, PartialEq)]
#[spacetimedb::table(name = skill_cooldown, public)]
pub struct SkillCooldown {
    /// surrogate PK for upserts and efficient lookups
    #[primary_key]
    #[auto_inc]
    pub id: u64,

    /// index for filter-based lookup
    #[index(btree)]
    pub player_id: Identity,
    #[index(btree)]
    pub skill_id: u8,

    pub last_used_at: Timestamp,
    pub base_cooldown: u32,
}