//! Session tracker for bridged CPDLC connections.
//!
//! Maintains the state needed to:
//! - Map between Hoppie and OpenLink MIN sequences.
//! - Track which callsign pairs have active bridged sessions.
//! - Prevent duplicate bridging of messages.
//! - Assign bridge-side MINs in the 1–63 circular range.

use std::collections::{HashMap, HashSet};

use tracing::debug;

/// Identifies one side of a CPDLC session.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionKey {
    /// Aircraft callsign.
    pub aircraft: String,
    /// Ground station callsign.
    pub station: String,
}

impl SessionKey {
    /// Create a new session key.
    pub fn new(aircraft: &str, station: &str) -> Self {
        Self {
            aircraft: aircraft.to_uppercase(),
            station: station.to_uppercase(),
        }
    }
}

/// Tracks bridged sessions and MIN mappings.
pub struct SessionTracker {
    /// MIN counter per session key — used to assign bridge-side MINs.
    min_counters: HashMap<SessionKey, u8>,
    /// Maps (session_key, source_side_min) → bridge_side_min.
    /// Used for MRN translation when relaying responses.
    min_map_hoppie_to_openlink: HashMap<(SessionKey, u8), u8>,
    min_map_openlink_to_hoppie: HashMap<(SessionKey, u8), u8>,
    /// Recently processed message ids to prevent re-bridging.
    seen_openlink_ids: HashSet<String>,
    /// Recently seen Hoppie messages (hash of from + packet) to deduplicate.
    seen_hoppie_hashes: HashSet<u64>,
}

impl SessionTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self {
            min_counters: HashMap::new(),
            min_map_hoppie_to_openlink: HashMap::new(),
            min_map_openlink_to_hoppie: HashMap::new(),
            seen_openlink_ids: HashSet::new(),
            seen_hoppie_hashes: HashSet::new(),
        }
    }

    /// Allocate the next MIN for a given session on the bridge side.
    /// Cycles 1–63 as per CPDLC spec.
    pub fn next_min(&mut self, key: &SessionKey) -> u8 {
        let counter = self.min_counters.entry(key.clone()).or_insert(0);
        *counter = (*counter % 63) + 1;
        *counter
    }

    /// Record a MIN mapping: Hoppie MIN → bridge MIN (for OpenLink side).
    pub fn record_hoppie_min(&mut self, key: &SessionKey, hoppie_min: u8, bridge_min: u8) {
        debug!(
            aircraft = %key.aircraft,
            station = %key.station,
            hoppie_min,
            bridge_min,
            "recorded MIN mapping hoppie→openlink"
        );
        self.min_map_hoppie_to_openlink
            .insert((key.clone(), hoppie_min), bridge_min);
    }

    /// Record a MIN mapping: OpenLink MIN → bridge MIN (for Hoppie side).
    pub fn record_openlink_min(&mut self, key: &SessionKey, openlink_min: u8, bridge_min: u8) {
        debug!(
            aircraft = %key.aircraft,
            station = %key.station,
            openlink_min,
            bridge_min,
            "recorded MIN mapping openlink→hoppie"
        );
        self.min_map_openlink_to_hoppie
            .insert((key.clone(), openlink_min), bridge_min);
    }

    /// Translate a Hoppie-side MRN to the corresponding OpenLink-side MIN.
    pub fn translate_hoppie_mrn(&self, key: &SessionKey, hoppie_mrn: u8) -> Option<u8> {
        self.min_map_hoppie_to_openlink
            .get(&(key.clone(), hoppie_mrn))
            .copied()
    }

    /// Translate an OpenLink-side MRN to the corresponding Hoppie-side MIN.
    pub fn translate_openlink_mrn(&self, key: &SessionKey, openlink_mrn: u8) -> Option<u8> {
        self.min_map_openlink_to_hoppie
            .get(&(key.clone(), openlink_mrn))
            .copied()
    }

    /// Check if an OpenLink message id has already been processed.
    pub fn is_openlink_seen(&self, id: &str) -> bool {
        self.seen_openlink_ids.contains(id)
    }

    /// Mark an OpenLink message id as processed.
    pub fn mark_openlink_seen(&mut self, id: &str) {
        self.seen_openlink_ids.insert(id.to_string());
        // Limit the set size to prevent unbounded growth.
        if self.seen_openlink_ids.len() > 10_000 {
            self.seen_openlink_ids.clear();
        }
    }

    /// Check if a Hoppie message has already been processed (by content hash).
    pub fn is_hoppie_seen(&self, hash: u64) -> bool {
        self.seen_hoppie_hashes.contains(&hash)
    }

    /// Mark a Hoppie message hash as processed.
    pub fn mark_hoppie_seen(&mut self, hash: u64) {
        self.seen_hoppie_hashes.insert(hash);
        if self.seen_hoppie_hashes.len() > 10_000 {
            self.seen_hoppie_hashes.clear();
        }
    }

    /// Reset all state for a given session (e.g. on disconnect).
    pub fn reset_session(&mut self, key: &SessionKey) {
        self.min_counters.remove(key);
        self.min_map_hoppie_to_openlink
            .retain(|(k, _), _| k != key);
        self.min_map_openlink_to_hoppie
            .retain(|(k, _), _| k != key);
    }
}

impl Default for SessionTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a simple hash for deduplication of Hoppie messages.
pub fn hoppie_message_hash(from: &str, packet: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    from.hash(&mut hasher);
    packet.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_cycles_1_to_63() {
        let mut tracker = SessionTracker::new();
        let key = SessionKey::new("AFR123", "LFPG");

        for expected in 1..=63 {
            assert_eq!(tracker.next_min(&key), expected);
        }
        // Should wrap back to 1
        assert_eq!(tracker.next_min(&key), 1);
    }

    #[test]
    fn min_mapping_roundtrip() {
        let mut tracker = SessionTracker::new();
        let key = SessionKey::new("AFR123", "LFPG");

        tracker.record_hoppie_min(&key, 5, 10);
        assert_eq!(tracker.translate_hoppie_mrn(&key, 5), Some(10));
        assert_eq!(tracker.translate_hoppie_mrn(&key, 3), None);
    }

    #[test]
    fn dedup_openlink() {
        let mut tracker = SessionTracker::new();
        assert!(!tracker.is_openlink_seen("msg-1"));
        tracker.mark_openlink_seen("msg-1");
        assert!(tracker.is_openlink_seen("msg-1"));
    }

    #[test]
    fn dedup_hoppie() {
        let mut tracker = SessionTracker::new();
        let hash = hoppie_message_hash("AFR123", "/data2/LFPG/WU/20//CLIMB");
        assert!(!tracker.is_hoppie_seen(hash));
        tracker.mark_hoppie_seen(hash);
        assert!(tracker.is_hoppie_seen(hash));
    }

    #[test]
    fn reset_clears_session() {
        let mut tracker = SessionTracker::new();
        let key = SessionKey::new("AFR123", "LFPG");
        tracker.next_min(&key);
        tracker.record_hoppie_min(&key, 1, 1);
        tracker.reset_session(&key);
        assert_eq!(tracker.next_min(&key), 1); // counter reset
        assert_eq!(tracker.translate_hoppie_mrn(&key, 1), None); // mapping gone
    }
}
