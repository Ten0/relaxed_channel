# relaxed_channel

**Wrapper around async-channel, more performant on heavy pipelines**

[![Crates.io](https://img.shields.io/crates/v/relaxed_channel.svg)](https://crates.io/crates/relaxed_channel)
[![License](https://img.shields.io/github/license/Ten0/relaxed_channel)](LICENSE)

## The issue

When readers (holding the `Receiver`) are faster than writers (holding the `Sender`), the channel's length will always be low. Normally, after reading a message, a reader task would encounter an empty channel when attempting to read the next message, and immediately set itself waiting for a wake up when the next message is sent. This implies that upon sending the next message, the writer has to dedicate a significant portion of its time marking the receiving tasks for wake up in the tokio runtime.

(The portion of time is significant when the pipeline gets heavy, around millions of messages per second.)

The effect increases as the number of readers increases (because there are more readers to wake up), so the more parallelism is required on the receiving end, the slower the writer(s) get.

## Solution

When reading a message and encountering an empty channel:

- Channel is empty => this means we are processing faster than the writers can produce data.
- Let's not cost time to the reader, and instead of marking ourselves for wake up, just sleep for e.g. 100ms.
- After 100ms, do a regular `Receiver::recv()` to check for a message, and only now mark ourselves for wake up if there is still no message available.

This implies that each reader will only have to be woken up by a writer at most once every 100ms.

This crate provides `RelaxedReceiver`, wrapping the regular `async_channel::Receiver` to add this behavior.

**Note that this design is such that all readers may be sleeping at the same time for 100ms, so in order for this to not slow the reader down, the channel's capacity must be large enough to hold more messages than may arrive during that time.**
