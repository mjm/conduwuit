pub(crate) mod data;
use std::{
	collections::HashSet,
	mem::size_of,
	sync::{Arc, Mutex},
};

pub(crate) use data::Data;
use lru_cache::LruCache;
use ruma::{EventId, RoomId};

use self::data::StateDiff;
use crate::{services, utils, Result};

type StateInfoLruCache = Mutex<
	LruCache<
		u64,
		Vec<(
			u64,                                // sstatehash
			Arc<HashSet<CompressedStateEvent>>, // full state
			Arc<HashSet<CompressedStateEvent>>, // added
			Arc<HashSet<CompressedStateEvent>>, // removed
		)>,
	>,
>;

type ShortStateInfoResult = Result<
	Vec<(
		u64,                                // sstatehash
		Arc<HashSet<CompressedStateEvent>>, // full state
		Arc<HashSet<CompressedStateEvent>>, // added
		Arc<HashSet<CompressedStateEvent>>, // removed
	)>,
>;

type ParentStatesVec = Vec<(
	u64,                                // sstatehash
	Arc<HashSet<CompressedStateEvent>>, // full state
	Arc<HashSet<CompressedStateEvent>>, // added
	Arc<HashSet<CompressedStateEvent>>, // removed
)>;

type HashSetCompressStateEvent = Result<(u64, Arc<HashSet<CompressedStateEvent>>, Arc<HashSet<CompressedStateEvent>>)>;

pub(crate) struct Service {
	pub(crate) db: &'static dyn Data,

	pub(crate) stateinfo_cache: StateInfoLruCache,
}

pub(crate) type CompressedStateEvent = [u8; 2 * size_of::<u64>()];

impl Service {
	/// Returns a stack with info on shortstatehash, full state, added diff and
	/// removed diff for the selected shortstatehash and each parent layer.
	#[tracing::instrument(skip(self))]
	pub(crate) fn load_shortstatehash_info(&self, shortstatehash: u64) -> ShortStateInfoResult {
		if let Some(r) = self
			.stateinfo_cache
			.lock()
			.unwrap()
			.get_mut(&shortstatehash)
		{
			return Ok(r.clone());
		}

		let StateDiff {
			parent,
			added,
			removed,
		} = self.db.get_statediff(shortstatehash)?;

		if let Some(parent) = parent {
			let mut response = self.load_shortstatehash_info(parent)?;
			let mut state = (*response.last().unwrap().1).clone();
			state.extend(added.iter().copied());
			let removed = (*removed).clone();
			for r in &removed {
				state.remove(r);
			}

			response.push((shortstatehash, Arc::new(state), added, Arc::new(removed)));

			self.stateinfo_cache
				.lock()
				.unwrap()
				.insert(shortstatehash, response.clone());

			Ok(response)
		} else {
			let response = vec![(shortstatehash, added.clone(), added, removed)];
			self.stateinfo_cache
				.lock()
				.unwrap()
				.insert(shortstatehash, response.clone());
			Ok(response)
		}
	}

	pub(crate) fn compress_state_event(&self, shortstatekey: u64, event_id: &EventId) -> Result<CompressedStateEvent> {
		let mut v = shortstatekey.to_be_bytes().to_vec();
		v.extend_from_slice(
			&services()
				.rooms
				.short
				.get_or_create_shorteventid(event_id)?
				.to_be_bytes(),
		);
		Ok(v.try_into().expect("we checked the size above"))
	}

	/// Returns shortstatekey, event id
	pub(crate) fn parse_compressed_state_event(
		&self, compressed_event: &CompressedStateEvent,
	) -> Result<(u64, Arc<EventId>)> {
		Ok((
			utils::u64_from_bytes(&compressed_event[0..size_of::<u64>()]).expect("bytes have right length"),
			services().rooms.short.get_eventid_from_short(
				utils::u64_from_bytes(&compressed_event[size_of::<u64>()..]).expect("bytes have right length"),
			)?,
		))
	}

	/// Creates a new shortstatehash that often is just a diff to an already
	/// existing shortstatehash and therefore very efficient.
	///
	/// There are multiple layers of diffs. The bottom layer 0 always contains
	/// the full state. Layer 1 contains diffs to states of layer 0, layer 2
	/// diffs to layer 1 and so on. If layer n > 0 grows too big, it will be
	/// combined with layer n-1 to create a new diff on layer n-1 that's
	/// based on layer n-2. If that layer is also too big, it will recursively
	/// fix above layers too.
	///
	/// * `shortstatehash` - Shortstatehash of this state
	/// * `statediffnew` - Added to base. Each vec is shortstatekey+shorteventid
	/// * `statediffremoved` - Removed from base. Each vec is
	///   shortstatekey+shorteventid
	/// * `diff_to_sibling` - Approximately how much the diff grows each time
	///   for this layer
	/// * `parent_states` - A stack with info on shortstatehash, full state,
	///   added diff and removed diff for each parent layer
	#[tracing::instrument(skip(self, statediffnew, statediffremoved, diff_to_sibling, parent_states))]
	pub(crate) fn save_state_from_diff(
		&self, shortstatehash: u64, statediffnew: Arc<HashSet<CompressedStateEvent>>,
		statediffremoved: Arc<HashSet<CompressedStateEvent>>, diff_to_sibling: usize,
		mut parent_states: ParentStatesVec,
	) -> Result<()> {
		let diffsum = statediffnew.len() + statediffremoved.len();

		if parent_states.len() > 3 {
			// Number of layers
			// To many layers, we have to go deeper
			let parent = parent_states.pop().unwrap();

			let mut parent_new = (*parent.2).clone();
			let mut parent_removed = (*parent.3).clone();

			for removed in statediffremoved.iter() {
				if !parent_new.remove(removed) {
					// It was not added in the parent and we removed it
					parent_removed.insert(*removed);
				}
				// Else it was added in the parent and we removed it again. We
				// can forget this change
			}

			for new in statediffnew.iter() {
				if !parent_removed.remove(new) {
					// It was not touched in the parent and we added it
					parent_new.insert(*new);
				}
				// Else it was removed in the parent and we added it again. We
				// can forget this change
			}

			self.save_state_from_diff(
				shortstatehash,
				Arc::new(parent_new),
				Arc::new(parent_removed),
				diffsum,
				parent_states,
			)?;

			return Ok(());
		}

		if parent_states.is_empty() {
			// There is no parent layer, create a new state
			self.db.save_statediff(
				shortstatehash,
				StateDiff {
					parent: None,
					added: statediffnew,
					removed: statediffremoved,
				},
			)?;

			return Ok(());
		};

		// Else we have two options.
		// 1. We add the current diff on top of the parent layer.
		// 2. We replace a layer above

		let parent = parent_states.pop().unwrap();
		let parent_diff = parent.2.len() + parent.3.len();

		if diffsum * diffsum >= 2 * diff_to_sibling * parent_diff {
			// Diff too big, we replace above layer(s)
			let mut parent_new = (*parent.2).clone();
			let mut parent_removed = (*parent.3).clone();

			for removed in statediffremoved.iter() {
				if !parent_new.remove(removed) {
					// It was not added in the parent and we removed it
					parent_removed.insert(*removed);
				}
				// Else it was added in the parent and we removed it again. We
				// can forget this change
			}

			for new in statediffnew.iter() {
				if !parent_removed.remove(new) {
					// It was not touched in the parent and we added it
					parent_new.insert(*new);
				}
				// Else it was removed in the parent and we added it again. We
				// can forget this change
			}

			self.save_state_from_diff(
				shortstatehash,
				Arc::new(parent_new),
				Arc::new(parent_removed),
				diffsum,
				parent_states,
			)?;
		} else {
			// Diff small enough, we add diff as layer on top of parent
			self.db.save_statediff(
				shortstatehash,
				StateDiff {
					parent: Some(parent.0),
					added: statediffnew,
					removed: statediffremoved,
				},
			)?;
		}

		Ok(())
	}

	/// Returns the new shortstatehash, and the state diff from the previous
	/// room state
	pub(crate) fn save_state(
		&self, room_id: &RoomId, new_state_ids_compressed: Arc<HashSet<CompressedStateEvent>>,
	) -> HashSetCompressStateEvent {
		let previous_shortstatehash = services().rooms.state.get_room_shortstatehash(room_id)?;

		let state_hash = utils::calculate_hash(
			&new_state_ids_compressed
				.iter()
				.map(|bytes| &bytes[..])
				.collect::<Vec<_>>(),
		);

		let (new_shortstatehash, already_existed) = services()
			.rooms
			.short
			.get_or_create_shortstatehash(&state_hash)?;

		if Some(new_shortstatehash) == previous_shortstatehash {
			return Ok((new_shortstatehash, Arc::new(HashSet::new()), Arc::new(HashSet::new())));
		}

		let states_parents =
			previous_shortstatehash.map_or_else(|| Ok(Vec::new()), |p| self.load_shortstatehash_info(p))?;

		let (statediffnew, statediffremoved) = if let Some(parent_stateinfo) = states_parents.last() {
			let statediffnew: HashSet<_> = new_state_ids_compressed
				.difference(&parent_stateinfo.1)
				.copied()
				.collect();

			let statediffremoved: HashSet<_> = parent_stateinfo
				.1
				.difference(&new_state_ids_compressed)
				.copied()
				.collect();

			(Arc::new(statediffnew), Arc::new(statediffremoved))
		} else {
			(new_state_ids_compressed, Arc::new(HashSet::new()))
		};

		if !already_existed {
			self.save_state_from_diff(
				new_shortstatehash,
				statediffnew.clone(),
				statediffremoved.clone(),
				2, // every state change is 2 event changes on average
				states_parents,
			)?;
		};

		Ok((new_shortstatehash, statediffnew, statediffremoved))
	}
}
