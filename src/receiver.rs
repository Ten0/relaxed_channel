use futures::prelude::*;

pub use async_channel::{self, Receiver, RecvError, TryRecvError};

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
		Self::relaxing_for(receiver, crate::DEFAULT_RELAXATION)
	}

	/// Wrap a [`async_channel::Receiver`] with a chosen relaxation
	pub fn relaxing_for(receiver: Receiver<T>, relax_for: std::time::Duration) -> Self {
		Self { receiver, relax_for }
	}

	/// Receives a message from the channel
	///
	/// If the channel is still empty, this method waits until there is a message.
	/// It may however take up to [`relax_for`](RelaxedReceiver::relaxing_for) for it to wake up after a message is made
	/// available by a [`Sender`](crate::Sender). (This guarantees that the task sending messages is not slowed down by
	/// the need to often wake up this receiving task.)
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

	/// Receives a message from the channel (blocking)
	///
	/// If the channel is still empty, this method waits until there is a message.
	/// It may however take up to [`relax_for`](RelaxedReceiver::relaxing_for) for it to wake up after a message is made
	/// available by a [`Sender`]. (This guarantees that the task sending messages is not slowed down by the need to
	/// often wake up this receiving task.)
	///
	/// If the channel is closed, this method receives a message or returns an error if there are no more messages.
	pub fn recv_blocking(&self) -> Result<T, RecvError> {
		match self.receiver.try_recv() {
			Ok(msg) => Ok(msg),
			Err(TryRecvError::Closed) => Err(RecvError),
			Err(TryRecvError::Empty) => {
				std::thread::sleep(self.relax_for);
				self.receiver.recv_blocking()
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

	/// Returns the channel capacity if it's bounded
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
