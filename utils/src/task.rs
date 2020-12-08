use crate::collections::FlaggedStorage;
use std::fmt::Debug;
use std::sync::{atomic::{AtomicUsize, Ordering}, Arc, Mutex};
use threadpool::ThreadPool;

#[derive(Debug, Clone)]
pub struct TaskPool {
    executor: Arc<ThreadPool>,
}

#[derive(Debug)]
pub struct ManagedTaskPool<T: 'static + Debug + Sized + Send + Sync> {
    executor: TaskPool,
    jobs: FlaggedStorage<Signal<T>>,
}

impl<T: 'static + Debug + Sized + Send + Sync> From<TaskPool> for ManagedTaskPool<T> {
    fn from(executor: TaskPool) -> Self {
        Self {
            executor,
            jobs: FlaggedStorage::new(),
        }
    }
}

impl<T: 'static + Debug + Sized + Send + Sync> From<&TaskPool> for ManagedTaskPool<T> {
    fn from(executor: &TaskPool) -> Self {
        Self {
            executor: executor.clone(),
            jobs: FlaggedStorage::new(),
        }
    }
}

#[derive(Debug)]
pub struct Signal<T: 'static + Debug + Sized + Send + Sync> {
    finished: Arc<AtomicUsize>,
    payload: Arc<Mutex<Option<T>>>,
}

impl<T: 'static + Debug + Sized + Send + Sync> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            finished: self.finished.clone(),
            payload: self.payload.clone(),
        }
    }
}

impl<T: 'static + Debug + Sized + Send + Sync> Default for Signal<T> {
    fn default() -> Self {
        Self {
            finished: Arc::new(AtomicUsize::new(0)),
            payload: Arc::new(Mutex::new(None)),
        }
    }
}

impl<T: 'static + Debug + Sized + Send + Sync> Signal<T> {
    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::Acquire) == 0
    }

    pub fn join(self) -> Option<T> {
        loop {
            if self.finished() {
                return if let Ok(mut payload) = self.payload.lock() {
                    payload.take()
                } else {
                    None
                };
            }
        }
    }
}

#[derive(Debug)]
pub struct Finish<T: 'static + Debug + Sized + Send + Sync> {
    finished: Arc<AtomicUsize>,
    payload: Arc<Mutex<Option<T>>>,
}

impl<T: 'static + Debug + Sized + Send + Sync> Finish<T> {
    pub fn new() -> (Self, Signal<T>) {
        let finished = Arc::new(AtomicUsize::new(1));
        let payload = Arc::new(Mutex::new(None));

        (
            Finish {
                finished: finished.clone(),
                payload: payload.clone(),
            },
            Signal { finished, payload },
        )
    }

    pub fn send(self, val: T) {
        if let Ok(mut payload) = self.payload.lock() {
            *payload = Some(val);
        }
    }
}

impl<T: 'static + Debug + Sized + Send + Sync> Drop for Finish<T> {
    fn drop(&mut self) {
        self.finished.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Default for TaskPool {
    fn default() -> Self {
        Self {
            executor: Arc::new(ThreadPool::new(num_cpus::get())),
        }
    }
}

impl TaskPool {
    pub fn new(nr_threads: usize) -> Self {
        Self {
            executor: Arc::new(ThreadPool::new(nr_threads)),
        }
    }

    pub fn push<F, T: 'static + Debug + Sized + Send + Sync>(&mut self, job: F) -> Signal<T>
        where
            F: FnOnce(Finish<T>) + Send + 'static,
    {
        let (finish, signal) = Finish::new();
        self.executor.execute(move || job(finish));
        signal
    }
}

impl<T: 'static + Debug + Sized + Send + Sync> Default for ManagedTaskPool<T> {
    fn default() -> Self {
        Self {
            executor: Default::default(),
            jobs: FlaggedStorage::new(),
        }
    }
}

impl<T: 'static + Debug + Sized + Send + Sync> ManagedTaskPool<T> {
    pub fn new(nr_threads: usize) -> Self {
        Self {
            executor: TaskPool::new(nr_threads),
            jobs: FlaggedStorage::new(),
        }
    }

    pub fn push<F>(&mut self, job: F)
        where
            F: FnOnce(Finish<T>) + Send + 'static,
    {
        let signal = self.executor.push(job);
        self.jobs.push(signal);
    }

    pub fn iter_finished(&mut self) -> FinishedIter<'_, T> {
        FinishedIter {
            jobs: &mut self.jobs,
            index: 0,
        }
    }

    pub fn sync(&mut self) -> SyncIter<'_, T> {
        SyncIter {
            jobs: &mut self.jobs,
            index: 0,
        }
    }
}

pub struct SyncIter<'a, T: 'static + Debug + Sized + Send + Sync> {
    jobs: &'a mut FlaggedStorage<Signal<T>>,
    index: usize,
}

impl<'a, T: 'static + Debug + Sized + Send + Sync> Iterator for SyncIter<'a, T> {
    type Item = Option<T>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.index >= self.jobs.len() {
                return None;
            }

            let index = self.index;
            self.index += 1;

            if let Some(_) = self.jobs.get(index) {
                let signal: Signal<T> = self.jobs.take(index).unwrap();
                return Some(signal.join());
            }
        }
    }
}

pub struct FinishedIter<'a, T: 'static + Debug + Sized + Send + Sync> {
    jobs: &'a mut FlaggedStorage<Signal<T>>,
    index: usize,
}

impl<'a, T: 'static + Debug + Sized + Send + Sync> Iterator for FinishedIter<'a, T> {
    type Item = Option<T>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.index >= self.jobs.len() {
                return None;
            }

            let index = self.index;
            self.index += 1;

            if let Some(_) = self.jobs.get(index) {
                if !self.jobs[index].finished() {
                    continue;
                }

                let signal: Signal<T> = self.jobs.take(index).unwrap();
                return Some(signal.join());
            }
        }
    }
}