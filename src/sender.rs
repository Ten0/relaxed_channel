pub use async_channel::{self, SendError, Sender};

use async_channel::TrySendError;

/// Wrapper around [`async_channel::Sender`] that sleeps for a bit when sending to a full channel
///
/// ...so as to not impact the performance of the task receiving messages.
///
/// See [crate documentation](crate) for more details.
#[derive(Debug)]
pub struct RelaxedSender<T> {
	sender: Sender<T>,
	relax_for: std::time::Duration,
}

impl<T> Clone for RelaxedSender<T> {
	fn clone(&self) -> Self {
		Self {
			sender: self.sender.clone(),
			relax_for: self.relax_for,
		}
	}
}

impl<T> RelaxedSender<T> {
	/// Wrap a [`async_channel::Sender`] with a 100ms relaxation
	pub fn new(sender: Sender<T>) -> Self {
		Self::relaxing_for(sender, crate::DEFAULT_RELAXATION)
	}

	/// Wrap a [`async_channel::Sender`] with a chosen relaxation
	pub fn relaxing_for(sender: Sender<T>, relax_for: std::time::Duration) -> Self {
		Self { sender, relax_for }
	}

	/// Sends a message inside the channel
	///
	/// If the channel is still full, this method waits until there is room to send it.
	/// It may however take up to [`relax_for`](RelaxedSender::relaxing_for) for it to wake up after a spot is made
	/// available by a [`Receiver`](crate::Receiver). (This guarantees that the task sending messages is not slowed down
	/// by the need to often wake up this receiving task.)
	///
	/// If the channel is closed, this method returns an error.
	pub async fn send(&self, msg: T) -> Result<(), SendError<T>> {
		match self.sender.try_send(msg) {
			Ok(()) => Ok(()),
			Err(TrySendError::Closed(msg)) => Err(SendError(msg)),
			Err(TrySendError::Full(msg)) => {
				tokio::time::sleep(self.relax_for).await;
				self.sender.send(msg).await
			}
		}
	}

	/// Sends a message inside the channel
	///
	/// If the channel is still full, this method waits until there is room to send it.
	/// It may however take up to [`relax_for`](RelaxedSender::relaxing_for) for it to wake up after a spot is made
	/// available by a [`Receiver`](crate::Receiver). (This guarantees that the task sending messages is not slowed down
	/// by the need to often wake up this receiving task.)
	///
	/// If the channel is closed, this method returns an error.
	pub fn send_blocking(&self, msg: T) -> Result<(), SendError<T>> {
		match self.sender.try_send(msg) {
			Ok(()) => Ok(()),
			Err(TrySendError::Closed(msg)) => Err(SendError(msg)),
			Err(TrySendError::Full(msg)) => {
				std::thread::sleep(self.relax_for);
				self.sender.send_blocking(msg)
			}
		}
	}

	/// Returns the number of messages in the channel
	pub fn len(&self) -> usize {
		self.sender.len()
	}

	/// Returns the channel capacity if it's bounded
	pub fn capacity(&self) -> Option<usize> {
		self.sender.capacity()
	}

	/// Obtain back the original [`async_channel::Sender`]
	pub fn into_inner(self) -> Sender<T> {
		self.sender
	}

	/// Get a reference to the original [`async_channel::Sender`]
	///
	/// That may be useful to call more advanced methods of [`async_channel::Sender`].
	pub fn inner(&self) -> &Sender<T> {
		&self.sender
	}

	/// Returns the relaxation duration
	pub fn relaxes_for(&self) -> std::time::Duration {
		self.relax_for
	}
}
