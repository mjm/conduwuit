use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use ruma::{events::StateEventType, EventId, RoomId};

use crate::{PduEvent, Result};

#[async_trait]
pub(crate) trait Data: Send + Sync {
	/// Builds a StateMap by iterating over all keys that start
	/// with state_hash, this gives the full state for the given state_hash.
	#[allow(unused_qualifications)] // async traits
	async fn state_full_ids(&self, shortstatehash: u64) -> Result<HashMap<u64, Arc<EventId>>>;

	#[allow(unused_qualifications)] // async traits
	async fn state_full(&self, shortstatehash: u64) -> Result<HashMap<(StateEventType, String), Arc<PduEvent>>>;

	/// Returns a single PDU from `room_id` with key (`event_type`,
	/// `state_key`).
	fn state_get_id(
		&self, shortstatehash: u64, event_type: &StateEventType, state_key: &str,
	) -> Result<Option<Arc<EventId>>>;

	/// Returns a single PDU from `room_id` with key (`event_type`,
	/// `state_key`).
	fn state_get(
		&self, shortstatehash: u64, event_type: &StateEventType, state_key: &str,
	) -> Result<Option<Arc<PduEvent>>>;

	/// Returns the state hash for this pdu.
	fn pdu_shortstatehash(&self, event_id: &EventId) -> Result<Option<u64>>;

	/// Returns the full room state.
	#[allow(unused_qualifications)] // async traits
	async fn room_state_full(&self, room_id: &RoomId) -> Result<HashMap<(StateEventType, String), Arc<PduEvent>>>;

	/// Returns a single PDU from `room_id` with key (`event_type`,
	/// `state_key`).
	fn room_state_get_id(
		&self, room_id: &RoomId, event_type: &StateEventType, state_key: &str,
	) -> Result<Option<Arc<EventId>>>;

	/// Returns a single PDU from `room_id` with key (`event_type`,
	/// `state_key`).
	fn room_state_get(
		&self, room_id: &RoomId, event_type: &StateEventType, state_key: &str,
	) -> Result<Option<Arc<PduEvent>>>;
}
