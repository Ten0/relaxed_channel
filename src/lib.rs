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
//! - Let's not cost time to the reader, and instead of marking ourselves for wake up, just sleep for e.g. 100ms.
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

use {async_channel::Receiver, futures::prelude::*};

pub use async_channel::{self, RecvError, Sender, TryRecvError};

const DEFAULT_RELAXATION: std::time::Duration = std::time::Duration::from_millis(100);

/// Construct a channel with the given capacity, and 100ms relaxation
pub fn bounded<T>(cap: usize) -> (Sender<T>, RelaxedReceiver<T>) {
	bounded_relaxing_for(cap, DEFAULT_RELAXATION)
}

/// Construct an unbounded channel with 100ms relaxation
pub fn unbounded<T>() -> (Sender<T>, RelaxedReceiver<T>) {
	unbounded_relaxing_for(DEFAULT_RELAXATION)
}

/// Construct a channel with the given capacity, and 100ms relaxation
pub fn bounded_relaxing_for<T>(cap: usize, relax_for: std::time::Duration) -> (Sender<T>, RelaxedReceiver<T>) {
	let (s, r) = async_channel::bounded(cap);
	(s, RelaxedReceiver::relaxing_for(r, relax_for))
}

/// Construct an unbounded channel with 100ms relaxation
pub fn unbounded_relaxing_for<T>(relax_for: std::time::Duration) -> (Sender<T>, RelaxedReceiver<T>) {
	let (s, r) = async_channel::unbounded();
	(s, RelaxedReceiver::relaxing_for(r, relax_for))
}

/// Wrapper around [`async_channel::Receiver`] that sleeps for a bit when receiving from an empty channel
///
/// ...so as to not impact the performance of the task sending messages.
///
/// See [crate documentation](crate) for more details.
#[derive(Debug)]
pub struct RelaxedReceiver<T> {
	receiver: Receiver<T>,
	relax_for: std::time::Duration,
}

impl<T> Clone for RelaxedReceiver<T> {
	fn clone(&self) -> Self {
		Self {
			receiver: self.receiver.clone(),
			relax_for: self.relax_for,
		}
	}
}

impl<T> RelaxedReceiver<T> {
	/// Wrap a [`async_channel::Receiver`] with a 100ms relaxation
	pub fn new(receiver: Receiver<T>) -> Self {
		Self::relaxing_for(receiver, DEFAULT_RELAXATION)
	}

	/// Wrap a [`async_channel::Receiver`] with a chosen relaxation
	pub fn relaxing_for(receiver: Receiver<T>, relax_for: std::time::Duration) -> Self {
		Self { receiver, relax_for }
	}

	/// Receives a message from the channel
	///
	/// If the channel is still empty, this method waits until there is a message.
	/// It may however take up to [`relax_for`](RelaxedReceiver::relaxing_for) for it to wake up after a message is made
	/// available by a [`Sender`]. (This guarantees that the task sending messages is not slowed down by the need to
	/// often wake up this receiving task.)
	///
	/// If the channel is closed, this method receives a message or returns an error if there are no more messages.
	pub async fn recv(&self) -> Result<T, RecvError> {
		match self.receiver.try_recv() {
			Ok(msg) => Ok(msg),
			Err(TryRecvError::Closed) => Err(RecvError),
			Err(TryRecvError::Empty) => {
				tokio::time::sleep(self.relax_for).await;
				self.receiver.recv().await
			}
		}
	}

	/// Obtains a Stream from this receiver
	///
	/// The returned stream is relaxed in the same way as [`RelaxedReceiver::recv`].
	pub fn stream<'a>(&'a self) -> impl Stream<Item = T> + 'a {
		stream::unfold((), move |()| self.recv().map(|res| res.ok().map(|msg| (msg, ()))))
	}

	/// Returns the number of messages in the channel
	pub fn len(&self) -> usize {
		self.receiver.len()
	}

	/// Returns the channel capacity if it’s bounded
	pub fn capacity(&self) -> Option<usize> {
		self.receiver.capacity()
	}

	/// Obtain back the original [`async_channel::Receiver`]
	pub fn into_inner(self) -> Receiver<T> {
		self.receiver
	}

	/// Get a reference to the original [`async_channel::Receiver`]
	///
	/// That may be useful to call more advanced methods of [`async_channel::Receiver`].
	pub fn inner(&self) -> &Receiver<T> {
		&self.receiver
	}
}
