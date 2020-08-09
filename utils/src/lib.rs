use crossbeam::{channel, Receiver, Sender};
use std::fmt::Debug;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use threadpool::ThreadPool;

#[derive(Debug)]
pub struct TaskPool<T: 'static + Debug + Sized + Send + Sync> {
    executor: ThreadPool,
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
            executor: ThreadPool::new(num_cpus::get()),
            jobs: Arc::new(AtomicUsize::new(0)),
            receiver,
            sender,
        }
    }
}

impl<T: 'static + Debug + Sized + Send + Sync> TaskPool<T> {
    pub fn new(nr_threads: usize) -> Self {
        Self {
            executor: ThreadPool::new(nr_threads),
            ..Default::default()
        }
    }

    pub fn push<F>(&mut self, job: F)
    where
        F: FnOnce(Finish<T>) + Send + 'static,
    {
        let jobs = self.jobs.clone();
        let sender = self.sender.clone();
        self.executor.execute(move || job(Finish { sender, jobs }));
        self.jobs.fetch_add(1, Ordering::AcqRel);
    }

    pub fn sync(&self) -> SyncIter<'_, T> {
        SyncIter {
            jobs: self.jobs.clone(),
            receiver: &self.receiver,
        }
    }

    pub fn take_finished(&self) -> FinishedIter<'_, T> {
        FinishedIter {
            jobs: self.jobs.clone(),
            receiver: &self.receiver,
        }
    }
}

pub struct SyncIter<'a, T: 'static + Debug + Sized + Send + Sync> {
    jobs: Arc<AtomicUsize>,
    receiver: &'a Receiver<T>,
}

impl<'a, T: 'static + Debug + Sized + Send + Sync> Iterator for SyncIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.jobs.load(Ordering::Acquire) != 0 || !self.receiver.is_empty() {
            if let Ok(val) = self.receiver.try_recv() {
                return Some(val);
            }
        }
        None
    }
}

pub struct FinishedIter<'a, T: 'static + Debug + Sized + Send + Sync> {
    jobs: Arc<AtomicUsize>,
    receiver: &'a Receiver<T>,
}

impl<'a, T: 'static + Debug + Sized + Send + Sync> Iterator for FinishedIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.receiver.is_empty() {
            if let Ok(val) = self.receiver.try_recv() {
                return Some(val);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
