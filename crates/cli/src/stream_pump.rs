// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Streams and futures carried from one store to another, the way
//! wasmtime-wasi carries a guest's standard input and output to the host
//! (`p3/cli/host.rs`, `InputStreamProducer` and `OutputStreamConsumer`): a
//! stream of bytes taken out of a store is written into one end of an
//! in-memory pipe by a [`ByteConsumer`], and a new stream in the other store
//! is fed from the pipe's other end by a [`ByteProducer`]. Each side moves
//! while its store's event loop runs. A future's value goes through a
//! oneshot channel. wasmtime's host API types what it carries: a stream of
//! bytes crosses, and a future of no value, a number, a bool or a char; the
//! build refuses the others.

use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, DuplexStream, ReadBuf};
use tokio::sync::oneshot;
use wasmtime::component::types::{FutureType, StreamType, Type};
use wasmtime::component::{
    ComponentType, Destination, FutureAny, FutureConsumer, FutureReader, Lift, Lower, Source,
    StreamAny, StreamConsumer, StreamProducer, StreamReader, StreamResult,
};
use wasmtime::{AsContextMut, Result, StoreContextMut, bail, format_err};

/// How many bytes the pipe holds, and a read delivers at most, as
/// wasmtime-wasi's `DEFAULT_BUFFER_CAPACITY`.
const PIPE_CAPACITY: usize = 8192;

/// The consumer the host attaches to a stream of bytes it takes out of a
/// store: what the store's writer sends goes into the pipe. The pipe's
/// reader gone, the stream is dropped for the writer.
struct ByteConsumer {
    pipe: DuplexStream,
}

impl<D> StreamConsumer<D> for ByteConsumer {
    type Item = u8;

    fn poll_consume(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        store: StoreContextMut<D>,
        src: Source<Self::Item>,
        finish: bool,
    ) -> Poll<Result<StreamResult>> {
        let mut src = src.as_direct(store);
        let buf = src.remaining();
        // An empty source asks whether the stream is writable: the pipe
        // cannot tell before a write, so it says yes, as wasmtime-wasi does.
        if buf.is_empty() {
            return Poll::Ready(Ok(StreamResult::Completed));
        }
        match Pin::new(&mut self.pipe).poll_write(cx, buf) {
            Poll::Ready(Ok(0)) | Poll::Ready(Err(_)) => Poll::Ready(Ok(StreamResult::Dropped)),
            Poll::Ready(Ok(n)) => {
                src.mark_read(n);
                Poll::Ready(Ok(StreamResult::Completed))
            }
            Poll::Pending if finish => Poll::Ready(Ok(StreamResult::Cancelled)),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// The producer of the stream the host makes in the other store: what the
/// pipe holds goes to the store's reader. The pipe's writer gone, the
/// stream ends.
struct ByteProducer {
    pipe: DuplexStream,
}

impl<D> StreamProducer<D> for ByteProducer {
    type Item = u8;
    type Buffer = Option<u8>;

    fn poll_produce<'a>(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut store: StoreContextMut<'a, D>,
        dst: Destination<'a, Self::Item, Self::Buffer>,
        finish: bool,
    ) -> Poll<Result<StreamResult>> {
        // An empty destination asks whether the stream is readable: the pipe
        // cannot tell before a read, so it says yes, as wasmtime-wasi does.
        if dst.remaining(store.as_context_mut()) == Some(0) {
            return Poll::Ready(Ok(StreamResult::Completed));
        }
        let mut dst = dst.as_direct(store, PIPE_CAPACITY);
        let mut buf = ReadBuf::new(dst.remaining());
        match Pin::new(&mut self.pipe).poll_read(cx, &mut buf) {
            Poll::Ready(Ok(())) if buf.filled().is_empty() => {
                Poll::Ready(Ok(StreamResult::Dropped))
            }
            Poll::Ready(Ok(())) => {
                let n = buf.filled().len();
                dst.mark_written(n);
                Poll::Ready(Ok(StreamResult::Completed))
            }
            Poll::Ready(Err(_)) => Poll::Ready(Ok(StreamResult::Dropped)),
            Poll::Pending if finish => Poll::Ready(Ok(StreamResult::Cancelled)),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// The reading end of the pipe a stream taken out of a store goes into.
pub struct StreamEnd(DuplexStream);

/// The stream `any`, of type `ty`, taken out of `store`: piped into a new
/// pipe, whose reading end it gives. Only a stream of bytes crosses.
pub fn take_stream<D: 'static>(
    store: &mut StoreContextMut<'_, D>,
    any: StreamAny,
    ty: &StreamType,
) -> Result<StreamEnd> {
    if !matches!(ty.ty(), Some(Type::U8)) {
        bail!("cross-store bridge: only a stream of bytes crosses between stores");
    }
    let reader = any.try_into_stream_reader::<u8>()?;
    let (writing, reading) = tokio::io::duplex(PIPE_CAPACITY);
    reader.pipe(&mut *store, ByteConsumer { pipe: writing })?;
    Ok(StreamEnd(reading))
}

/// A new stream of `store`'s, fed from the pipe `end` reads.
pub fn give_stream<D: 'static>(
    store: &mut StoreContextMut<'_, D>,
    end: StreamEnd,
) -> Result<StreamAny> {
    let reader = StreamReader::<u8>::new(&mut *store, ByteProducer { pipe: end.0 })?;
    StreamAny::try_from_stream_reader(&mut *store, reader)
}

/// The consumer the host attaches to a future it takes out of a store: the
/// value goes into the channel.
struct ValueConsumer<T> {
    sender: Option<oneshot::Sender<T>>,
}

impl<D, T: Lift + Send + Sync + 'static> FutureConsumer<D> for ValueConsumer<T> {
    type Item = T;

    fn poll_consume(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        store: StoreContextMut<D>,
        mut source: Source<'_, Self::Item>,
        _: bool,
    ) -> Poll<Result<()>> {
        let mut value = None;
        source.read(store, &mut value)?;
        if let (Some(value), Some(sender)) = (value, self.sender.take()) {
            let _ = sender.send(value);
        }
        Poll::Ready(Ok(()))
    }
}

/// The future `any` taken out of `store`, piped into a new channel, whose
/// receiving end it gives.
fn pipe_future<D: 'static, T>(
    store: &mut StoreContextMut<'_, D>,
    any: FutureAny,
) -> Result<oneshot::Receiver<T>>
where
    T: ComponentType + Lift + Lower + Send + Sync + 'static,
{
    let reader = any.try_into_future_reader::<T>()?;
    let (sender, receiver) = oneshot::channel();
    reader.pipe(
        &mut *store,
        ValueConsumer {
            sender: Some(sender),
        },
    )?;
    Ok(receiver)
}

/// A new future of `store`'s, whose value comes through `receiver`.
fn feed_future<D: 'static, T>(
    store: &mut StoreContextMut<'_, D>,
    receiver: oneshot::Receiver<T>,
) -> Result<FutureAny>
where
    T: ComponentType + Lift + Lower + Send + Sync + 'static,
{
    let reader = FutureReader::new(&mut *store, async move {
        receiver
            .await
            .map_err(|_| format_err!("cross-store bridge: the future's writer left with no value"))
    })?;
    FutureAny::try_from_future_reader(&mut *store, reader)
}

/// The futures that cross between stores, by the type of their value, and
/// the Rust type wasmtime's host API carries it as.
macro_rules! future_ends {
    ($($case:ident($rust:ty) = $wit:ident),* $(,)?) => {
        /// The receiving end of the channel a future's value goes through, by
        /// the value's type.
        pub enum FutureEnd {
            Unit(oneshot::Receiver<()>),
            $($case(oneshot::Receiver<$rust>),)*
        }

        /// The future `any`, of type `ty`, taken out of `store`: piped into
        /// a new channel, whose receiving end it gives. Only a future of no
        /// value, a number, a bool or a char crosses.
        pub fn take_future<D: 'static>(
            store: &mut StoreContextMut<'_, D>,
            any: FutureAny,
            ty: &FutureType,
        ) -> Result<FutureEnd> {
            Ok(match ty.ty() {
                None => FutureEnd::Unit(pipe_future::<D, ()>(store, any)?),
                $(Some(Type::$wit) => FutureEnd::$case(pipe_future::<D, $rust>(store, any)?),)*
                Some(_) => bail!(
                    "cross-store bridge: only a future of no value, a number, a bool or a char \
                     crosses between stores"
                ),
            })
        }

        /// A new future of `store`'s, whose value comes through `end`.
        pub fn give_future<D: 'static>(
            store: &mut StoreContextMut<'_, D>,
            end: FutureEnd,
        ) -> Result<FutureAny> {
            match end {
                FutureEnd::Unit(receiver) => feed_future(store, receiver),
                $(FutureEnd::$case(receiver) => feed_future(store, receiver),)*
            }
        }
    };
}

future_ends! {
    Bool(bool) = Bool,
    U8(u8) = U8,
    S8(i8) = S8,
    U16(u16) = U16,
    S16(i16) = S16,
    U32(u32) = U32,
    S32(i32) = S32,
    U64(u64) = U64,
    S64(i64) = S64,
    F32(f32) = Float32,
    F64(f64) = Float64,
    Char(char) = Char,
}
