use spacetimedb::Identity;
use std::fmt;
use std::borrow::Borrow;

/// Newtype wrapper for physics entity IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicsBodyId(pub Identity);

impl From<Identity> for PhysicsBodyId {
    fn from(id: Identity) -> Self {
        PhysicsBodyId(id)
    }
}
impl From<PhysicsBodyId> for Identity {
    fn from(nb: PhysicsBodyId) -> Self {
        nb.0
    }
}

impl fmt::Display for PhysicsBodyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_hex())
    }
}

impl Borrow<Identity> for PhysicsBodyId {
    fn borrow(&self) -> &Identity {
        &self.0
    }
}

/// Newtype for a pair of physics entities in contact
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContactPair(pub PhysicsBodyId, pub PhysicsBodyId);

impl ContactPair {
    /// Order pair consistently by ID
    pub fn new(a: PhysicsBodyId, b: PhysicsBodyId) -> Self {
        if a.0 < b.0 {
            ContactPair(a, b)
        } else {
            ContactPair(b, a)
        }
    }
}

impl Borrow<Identity> for ContactPair {
    fn borrow(&self) -> &Identity {
        &self.0 .0
    }
}