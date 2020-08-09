use crossbeam::{channel, Receiver, Sender};
use futures::task::SpawnExt;
use std::fmt::Debug;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(Debug)]
pub struct TaskPool<T: 'static + Debug + Sized + Send + Sync> {
    executor: futures_executor::ThreadPool,
    jobs: Arc<AtomicUsize>,
    receiver: Receiver<T>,
    sender: Sender<T>,
}

pub struct Finish<T: 'static + Debug + Sized + Send + Sync> {
    sender: Sender<T>,
    jobs: Arc<AtomicUsize>,
}

impl<T: 'static + Debug + Sized + Send + Sync> Finish<T> {
    pub fn send(self, val: T) {
        self.sender.send(val).unwrap();
    }
}

impl<T: 'static + Debug + Sized + Send + Sync> Drop for Finish<T> {
    fn drop(&mut self) {
        self.jobs.fetch_sub(1, Ordering::AcqRel);
    }
}

impl<T: 'static + Debug + Sized + Send + Sync> Default for TaskPool<T> {
    fn default() -> Self {
        let (sender, receiver) = channel::unbounded();

        Self {
            executor: futures_executor::ThreadPoolBuilder::new()
                .pool_size(num_cpus::get())
                .create()
                .unwrap(),
            jobs: Arc::new(AtomicUsize::new(0)),
            receiver,
            sender,
        }
    }
}

impl<T: 'static + Debug + Sized + Send + Sync> TaskPool<T> {
    pub fn new(nr_threads: usize) -> Self {
        Self {
            executor: futures_executor::ThreadPoolBuilder::new()
                .pool_size(nr_threads)
                .create()
                .unwrap(),
            ..Default::default()
        }
    }

    pub fn push<F>(&mut self, job: F)
    where
        F: FnOnce(Finish<T>) + Send + 'static,
    {
        self.jobs.fetch_add(1, Ordering::AcqRel);

        let jobs = self.jobs.clone();
        let sender = self.sender.clone();
        self.executor
            .spawn(async move { job(Finish { sender, jobs }) })
            .unwrap();
    }

    pub fn sync<F>(&mut self, mut cb: F)
    where
        F: FnMut(T),
    {
        while self.jobs.load(Ordering::Acquire) != 0 || !self.receiver.is_empty() {
            if let Ok(val) = self.receiver.try_recv() {
                cb(val)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
