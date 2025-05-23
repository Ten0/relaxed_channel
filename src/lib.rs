//! # relaxed_channel
//!
//! Wrapper around [`async_channel`], more performant on heavy pipelines
//!
//! ## The issue
//!
//! When readers (holding the [`Receiver`]) are faster than writers (holding the [`Sender`]), the channel's length will
//! always be low. Normally, after reading a message, a reader task would encounter an empty channel when
//! attempting to read the next message, and immediately set itself waiting for a wake up when the next message is sent.
//! This implies that upon sending the next message, the writer has to dedicate a significant portion of its time
//! marking the receiving tasks for wake up in the tokio runtime.
//!
//! (The portion of time is significant when the pipeline gets heavy, around millions of messages per second.)
//!
//! The effect increases as the number of readers increases (because there are more readers to wake up), so the more
//! parallelism is required on the receiving end, the slower the writer(s) get.
//!
//! ## Solution
//!
//! When reading a message and encountering an empty channel:
//!
//! - Channel is empty => this means we are processing faster than the writers can produce data.
//! - Let's not cost time to the writer, and instead of marking ourselves for wake up, just sleep for e.g. 100ms.
//! - After 100ms, do a regular [`Receiver::recv()`] to check for a message, and only now mark ourselves for wake up if
//!   there is still no message available.
//!
//! This implies that each reader will only have to be woken up by a writer at most once every 100ms.
//!
//! This crate provides [`RelaxedReceiver`], wrapping the regular [`async_channel::Receiver`] to add this behavior.
//!
//! **Note that this design is such that all readers may be sleeping at the same time for 100ms, so in order for this to
//! not slow the reader down, the channel's capacity must be large enough to hold more messages than may arrive during
//! that time.**
//!
//! ## Writers
//!
//! There's the symetrical issue with writers, when they are faster than readers and the channel can be full. This crate
//! also provides [`RelaxedSender`] to address this issue.
//!
//! The [`bounded`] function creates a channel relaxed on both ends.
//!
//! The [`unbounded`] function creates an unbounded channel relaxed on the receiver end only, because the sender can't
//! be blocked by the receiver if the channel is unbounded.
//!
//! You can construct both [`RelaxedSender`] and [`RelaxedReceiver`] separately if you need to customize their
//! relaxations individually.

mod receiver;
mod sender;

pub use {receiver::*, sender::*};

const DEFAULT_RELAXATION: std::time::Duration = std::time::Duration::from_millis(100);

/// Construct a channel with the given capacity, and 100ms relaxation
pub fn bounded<T>(cap: usize) -> (RelaxedSender<T>, RelaxedReceiver<T>) {
	bounded_relaxing_for(cap, DEFAULT_RELAXATION)
}

/// Construct an unbounded channel with 100ms relaxation
///
/// Channel is relaxed on the receiver end only, because the sender can't ever be blocked by the receiver if the channel
/// is unbounded.
pub fn unbounded<T>() -> (Sender<T>, RelaxedReceiver<T>) {
	unbounded_relaxing_for(DEFAULT_RELAXATION)
}

/// Construct a channel with the given capacity, and a chosen relaxation
pub fn bounded_relaxing_for<T>(cap: usize, relax_for: std::time::Duration) -> (RelaxedSender<T>, RelaxedReceiver<T>) {
	let (s, r) = async_channel::bounded(cap);
	(
		RelaxedSender::relaxing_for(s, relax_for),
		RelaxedReceiver::relaxing_for(r, relax_for),
	)
}

/// Construct an unbounded channel with a chosen relaxation
pub fn unbounded_relaxing_for<T>(relax_for: std::time::Duration) -> (Sender<T>, RelaxedReceiver<T>) {
	let (s, r) = async_channel::unbounded();
	(s, RelaxedReceiver::relaxing_for(r, relax_for))
}
