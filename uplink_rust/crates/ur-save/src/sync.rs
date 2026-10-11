//! Sync protocol shared by client and server, plus the pure decision logic.
//!
//! Model: each slot has a server-side monotonically increasing `revision`.
//! The client remembers the last revision it synced (`base`) and whether it
//! has local changes since (`dirty`). Pushes are compare-and-swap on `base`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotInfo {
    pub slot: String,
    pub revision: u64,
    pub size: u64,
    /// Unix seconds, server clock.
    pub updated_at: i64,
}

/// Per-slot client bookkeeping, stored next to the local save.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncState {
    /// Last server revision this device saw. 0 = never synced.
    pub base: u64,
    /// Local save changed since `base`.
    pub dirty: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Nothing,
    /// Upload; send `if_revision` as the expected current server revision.
    Push {
        if_revision: u64,
    },
    Pull,
    /// Both sides changed. Never auto-resolve: ask the player (keep local /
    /// keep remote / keep both as separate slots).
    Conflict,
}

/// `local`: does a local save exist. `remote`: server revision, if the slot exists.
pub fn reconcile(local: Option<SyncState>, remote: Option<u64>) -> Action {
    match (local, remote) {
        (None, None) => Action::Nothing,
        (None, Some(_)) => Action::Pull,
        (Some(l), None) => {
            // Never synced -> first upload. Previously synced but gone remotely -> server was reset.
            if l.base == 0 {
                Action::Push { if_revision: 0 }
            } else {
                Action::Conflict
            }
        }
        (Some(l), Some(r)) if r == l.base => {
            if l.dirty {
                Action::Push { if_revision: r }
            } else {
                Action::Nothing
            }
        }
        (Some(l), Some(r)) if r > l.base => {
            if l.dirty {
                Action::Conflict
            } else {
                Action::Pull
            }
        }
        // Remote is behind what we saw: server restored from backup or reset.
        (Some(_), Some(_)) => Action::Conflict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(base: u64, dirty: bool) -> Option<SyncState> {
        Some(SyncState { base, dirty })
    }

    #[test]
    fn decisions() {
        assert_eq!(reconcile(None, None), Action::Nothing);
        assert_eq!(reconcile(None, Some(3)), Action::Pull);
        assert_eq!(reconcile(s(0, true), None), Action::Push { if_revision: 0 });
        assert_eq!(reconcile(s(2, false), None), Action::Conflict);
        assert_eq!(
            reconcile(s(2, true), Some(2)),
            Action::Push { if_revision: 2 }
        );
        assert_eq!(reconcile(s(2, false), Some(2)), Action::Nothing);
        assert_eq!(reconcile(s(2, false), Some(5)), Action::Pull);
        assert_eq!(reconcile(s(2, true), Some(5)), Action::Conflict);
        assert_eq!(reconcile(s(5, false), Some(2)), Action::Conflict);
    }
}
