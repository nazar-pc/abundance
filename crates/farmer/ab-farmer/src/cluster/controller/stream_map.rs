//! A stream map that keeps track of futures that are currently being processed for each `Index`.

#[cfg(test)]
mod tests;

use futures::stream::FusedStream;
use futures::{FutureExt, Stream, StreamExt};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_stream::StreamMap as TokioStreamMap;

type TaskFuture<'a, R> = Pin<Box<dyn Future<Output = R> + 'a>>;
type TaskStream<'a, R> = Pin<Box<dyn Stream<Item = R> + Unpin + 'a>>;

/// A StreamMap that keeps track of futures that are currently being processed for each `index`.
pub(super) struct StreamMap<'a, Index, R> {
    in_progress: TokioStreamMap<Index, TaskStream<'a, R>>,
    queue: HashMap<Index, VecDeque<TaskFuture<'a, R>>>,
}

impl<Index, R> Default for StreamMap<'_, Index, R> {
    fn default() -> Self {
        Self {
            in_progress: TokioStreamMap::default(),
            queue: HashMap::default(),
        }
    }
}

impl<'a, Index, R: 'a> StreamMap<'a, Index, R>
where
    Index: Eq + Hash + Copy + Unpin,
{
    /// When pushing a new task, it first checks if there is already a future for the given `index`
    /// in `in_progress`.
    ///   - If there is, the task is added to `queue`.
    ///   - If not, the task is directly added to `in_progress`.
    pub(super) fn push(&mut self, index: Index, fut: TaskFuture<'a, R>) {
        if self.in_progress.contains_key(&index) {
            let queue = self.queue.entry(index).or_default();
            queue.push_back(fut);
        } else {
            self.in_progress
                .insert(index, Box::pin(fut.into_stream()) as _);
        }
    }

    /// Skip the task if there is already a future for the given `index` in `in_progress`.
    /// Returns `true` if the task is added to `in_progress`, `false` otherwise.
    pub(super) fn add_if_not_in_progress(&mut self, index: Index, fut: TaskFuture<'a, R>) -> bool {
        if self.in_progress.contains_key(&index) {
            false
        } else {
            self.in_progress
                .insert(index, Box::pin(fut.into_stream()) as _);
            true
        }
    }

    /// Polls the next entry in `in_progress` and moves the next task from `queue` to `in_progress`
    /// if there is one. If there are no more tasks to execute, returns `None`.
    fn poll_next_entry(&mut self, cx: &mut Context<'_>) -> Poll<Option<(Index, R)>> {
        if let Some((index, res)) = std::task::ready!(self.in_progress.poll_next_unpin(cx)) {
            // Current task completed, remove from in_progress queue and check for more tasks
            self.in_progress.remove(&index);
            self.process_queue(index);
            Poll::Ready(Some((index, res)))
        } else {
            // No more tasks to execute
            assert!(self.queue.is_empty());
            Poll::Ready(None)
        }
    }

    /// Process the next task from the tasks queue for the given `index`
    fn process_queue(&mut self, index: Index) {
        if let Entry::Occupied(mut next_entry) = self.queue.entry(index) {
            let task_queue = next_entry.get_mut();
            if let Some(fut) = task_queue.pop_front() {
                self.in_progress
                    .insert(index, Box::pin(fut.into_stream()) as _);
            }

            // Remove the index from the map if there are no more tasks
            if task_queue.is_empty() {
                next_entry.remove();
            }
        }
    }
}

impl<'a, Index, R: 'a> Stream for StreamMap<'a, Index, R>
where
    Index: Eq + Hash + Copy + Unpin,
{
    type Item = (Index, R);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        this.poll_next_entry(cx)
    }
}

impl<'a, Index, R: 'a> FusedStream for StreamMap<'a, Index, R>
where
    Index: Eq + Hash + Copy + Unpin,
{
    fn is_terminated(&self) -> bool {
        self.in_progress.is_empty() && self.queue.is_empty()
    }
}
