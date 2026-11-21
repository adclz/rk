// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Streams and futures carried from one store to another, the way wasmCloud's
//! `wash-runtime` carries them between its stores
//! (`engine/store/stream_pump.rs`): a `stream<T>` goes through a bounded
//! channel of chunks of items. In the store the stream leaves, a
//! [`ChannelConsumer`] is piped the stream's reader and sends on what the
//! writer offers once the channel has room for it; in the other store, a
//! [`ChannelProducer`] feeds a new stream from the channel. The channel holds
//! [`DEFAULT_CAPACITY`] chunks, so a writer faster than its reader waits
//! rather than fills memory, and nothing is held back until the stream ends.
//! A future's value goes through a oneshot channel ([`FutureSink`]).
//!
//! Where wasmCloud's pump waits, two answers follow wasmtime's contract for
//! `poll_produce` and `poll_consume`, as wasmtime-wasi's own streams do: asked
//! to finish with nothing to move, each says `Cancelled`, so a guest
//! cancelling a read or a write is not held up; and a read of no items is
//! answered at once, without taking a chunk the reader may never ask for.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::StreamExt as _;
use futures::channel::{mpsc, oneshot};
use wasmtime::component::{
    Destination, FutureConsumer, Lift, Lower, Source, StreamConsumer, StreamProducer, StreamResult,
    VecBuffer,
};
use wasmtime::{AsContextMut, Result, StoreContextMut, format_err};

/// How many chunks the channel between two stores holds before the writer
/// waits.
pub const DEFAULT_CAPACITY: usize = 8;

/// The store a stream goes to: a new stream there, fed from the channel.
/// The channel's sender gone, the stream ends.
pub struct ChannelProducer<T> {
    rx: mpsc::Receiver<Vec<T>>,
}

impl<T, D> StreamProducer<D> for ChannelProducer<T>
where
    T: Lower + Send + Sync + 'static,
{
    type Item = T;
    type Buffer = VecBuffer<T>;

    fn poll_produce<'a>(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut store: StoreContextMut<'a, D>,
        mut dst: Destination<'a, Self::Item, Self::Buffer>,
        finish: bool,
    ) -> Poll<Result<StreamResult>> {
        // A read of no items asks whether the stream is readable: it is
        // answered at once, and no chunk is taken that the reader may never
        // ask for.
        if dst.remaining(store.as_context_mut()) == Some(0) {
            return Poll::Ready(Ok(StreamResult::Completed));
        }
        match self.get_mut().rx.poll_next_unpin(cx) {
            Poll::Ready(Some(chunk)) => {
                dst.set_buffer(VecBuffer::from(chunk));
                Poll::Ready(Ok(StreamResult::Completed))
            }
            Poll::Ready(None) => Poll::Ready(Ok(StreamResult::Dropped)),
            Poll::Pending if finish => Poll::Ready(Ok(StreamResult::Cancelled)),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// The store a stream leaves: what its writer offers goes into the channel,
/// once the channel has room for a chunk. The channel's receiver gone, the
/// stream is dropped for the writer.
pub struct ChannelConsumer<T> {
    tx: mpsc::Sender<Vec<T>>,
}

impl<T, D: 'static> StreamConsumer<D> for ChannelConsumer<T>
where
    T: Lift + Send + Sync + 'static,
{
    type Item = T;

    fn poll_consume(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut store: StoreContextMut<D>,
        mut source: Source<'_, Self::Item>,
        finish: bool,
    ) -> Poll<Result<StreamResult>> {
        let this = self.get_mut();
        match this.tx.poll_ready(cx) {
            Poll::Ready(Ok(())) => {}
            Poll::Ready(Err(_)) => return Poll::Ready(Ok(StreamResult::Dropped)),
            Poll::Pending if finish => return Poll::Ready(Ok(StreamResult::Cancelled)),
            Poll::Pending => return Poll::Pending,
        }
        // A write of no items asks whether the stream is writable: the
        // channel has room.
        let available = source.remaining(&mut store);
        if available == 0 {
            return Poll::Ready(Ok(StreamResult::Completed));
        }
        let mut chunk: Vec<T> = Vec::with_capacity(available);
        source.read(&mut store, &mut chunk)?;
        if chunk.is_empty() {
            return Poll::Ready(Ok(StreamResult::Completed));
        }
        // The room `poll_ready` found is taken: this fails only when the
        // receiver left in between, which drops the stream rather than the
        // chunk without a word.
        if this.tx.start_send(chunk).is_err() {
            return Poll::Ready(Ok(StreamResult::Dropped));
        }
        Poll::Ready(Ok(StreamResult::Completed))
    }
}

/// A consumer and a producer joined by a channel of `capacity` chunks.
pub fn channel<T>(capacity: usize) -> (ChannelConsumer<T>, ChannelProducer<T>) {
    let (tx, rx) = mpsc::channel(capacity);
    (ChannelConsumer { tx }, ChannelProducer { rx })
}

/// The store a future leaves: its value goes through a oneshot channel,
/// whose receiver the new future in the other store awaits.
pub struct FutureSink<T> {
    tx: Option<oneshot::Sender<T>>,
}

impl<T, D: 'static> FutureConsumer<D> for FutureSink<T>
where
    T: Lift + Send + Sync + 'static,
{
    type Item = T;

    fn poll_consume(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        mut store: StoreContextMut<D>,
        mut source: Source<'_, Self::Item>,
        finish: bool,
    ) -> Poll<Result<()>> {
        let this = self.get_mut();
        if source.remaining(&mut store) == 0 {
            // The read was cancelled before a value came.
            if finish {
                return Poll::Ready(Ok(()));
            }
            // A future's write presents its value whenever the consumer is
            // polled: waiting here would hold the writer up with no waker to
            // wake it, so this fails loudly should that ever change.
            return Poll::Ready(Err(format_err!(
                "cross-store bridge: a future's consumer polled with no value"
            )));
        }
        let mut value: Vec<T> = Vec::with_capacity(1);
        source.read(&mut store, &mut value)?;
        if let (Some(value), Some(tx)) = (value.into_iter().next(), this.tx.take()) {
            let _ = tx.send(value);
        }
        Poll::Ready(Ok(()))
    }
}

/// A sink for a future's value, and the receiver the value reaches.
pub fn future_channel<T>() -> (FutureSink<T>, oneshot::Receiver<T>) {
    let (tx, rx) = oneshot::channel();
    (FutureSink { tx: Some(tx) }, rx)
}
