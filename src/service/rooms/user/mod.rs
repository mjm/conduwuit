mod data;

pub(crate) use data::Data;
use ruma::{OwnedRoomId, OwnedUserId, RoomId, UserId};

use crate::Result;

pub(crate) struct Service {
	pub(crate) db: &'static dyn Data,
}

impl Service {
	pub(crate) fn reset_notification_counts(&self, user_id: &UserId, room_id: &RoomId) -> Result<()> {
		self.db.reset_notification_counts(user_id, room_id)
	}

	pub(crate) fn notification_count(&self, user_id: &UserId, room_id: &RoomId) -> Result<u64> {
		self.db.notification_count(user_id, room_id)
	}

	pub(crate) fn highlight_count(&self, user_id: &UserId, room_id: &RoomId) -> Result<u64> {
		self.db.highlight_count(user_id, room_id)
	}

	pub(crate) fn last_notification_read(&self, user_id: &UserId, room_id: &RoomId) -> Result<u64> {
		self.db.last_notification_read(user_id, room_id)
	}

	pub(crate) fn associate_token_shortstatehash(
		&self, room_id: &RoomId, token: u64, shortstatehash: u64,
	) -> Result<()> {
		self.db
			.associate_token_shortstatehash(room_id, token, shortstatehash)
	}

	pub(crate) fn get_token_shortstatehash(&self, room_id: &RoomId, token: u64) -> Result<Option<u64>> {
		self.db.get_token_shortstatehash(room_id, token)
	}

	pub(crate) fn get_shared_rooms(
		&self, users: Vec<OwnedUserId>,
	) -> Result<impl Iterator<Item = Result<OwnedRoomId>>> {
		self.db.get_shared_rooms(users)
	}
}
