use crate::hal;
use crate::hal::device::Device;
use crate::hal::pool::CommandPool;
use crate::hal::queue::CommandQueue;
use hal::*;
use std::borrow::Borrow;
use std::fmt::Formatter;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
#[repr(transparent)]
pub struct DeviceHandle<B: hal::Backend>(Arc<B::Device>);

impl<B: hal::Backend> DeviceHandle<B> {
    pub fn new(device: B::Device) -> Self {
        Self(Arc::new(device))
    }
}

impl<B: hal::Backend> Clone for DeviceHandle<B> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<B: hal::Backend> Deref for DeviceHandle<B> {
    type Target = B::Device;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

#[derive(Debug)]
pub struct Queue<B: hal::Backend> {
    pub family: queue::QueueFamilyId,
    queue_group: Arc<Mutex<ManuallyDrop<queue::QueueGroup<B>>>>,
}

impl<B: hal::Backend> Clone for Queue<B> {
    fn clone(&self) -> Self {
        Self {
            family: self.family,
            queue_group: self.queue_group.clone(),
        }
    }
}

impl<B: hal::Backend> Queue<B> {
    pub fn new(group: queue::QueueGroup<B>) -> Self {
        Self {
            family: group.family,
            queue_group: Arc::new(Mutex::new(ManuallyDrop::new(group))),
        }
    }

    pub fn submit<'a, T, Ic, S, Iw, Is>(
        &self,
        submission: queue::Submission<Ic, Iw, Is>,
        fence: Option<&B::Fence>,
    ) where
        T: 'a + Borrow<B::CommandBuffer>,
        Ic: IntoIterator<Item = &'a T>,
        S: 'a + Borrow<B::Semaphore>,
        Iw: IntoIterator<Item = (&'a S, pso::PipelineStage)>,
        Is: IntoIterator<Item = &'a S>,
    {
        unsafe { self.queue_group.lock().unwrap().queues[0].submit(submission, fence) }
    }

    pub fn submit_without_semaphores<'a, T, Ic>(
        &self,
        command_buffers: Ic,
        fence: Option<&B::Fence>,
    ) where
        T: 'a + Borrow<B::CommandBuffer>,
        Ic: IntoIterator<Item = &'a T>,
    {
        let submission = queue::Submission {
            command_buffers,
            wait_semaphores: std::iter::empty(),
            signal_semaphores: std::iter::empty(),
        };
        self.submit::<_, _, B::Semaphore, _, _>(submission, fence)
    }

    pub fn present(
        &self,
        surface: &mut B::Surface,
        image: <B::Surface as window::PresentationSurface<B>>::SwapchainImage,
        wait_semaphore: Option<&B::Semaphore>,
    ) -> Result<Option<window::Suboptimal>, window::PresentError> {
        unsafe {
            self.queue_group.lock().unwrap().queues[0].present(surface, image, wait_semaphore)
        }
    }

    pub fn wait_idle(&mut self) -> Result<(), device::OutOfMemory> {
        self.queue_group.lock().unwrap().queues[0].wait_idle()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CommandError {
    PoolMissesCreationFlag(pool::CommandPoolCreateFlags),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Error {{ {} }}",
            match self {
                CommandError::PoolMissesCreationFlag(flag) => {
                    if flag.contains(pool::CommandPoolCreateFlags::RESET_INDIVIDUAL) {
                        format!("pool misses RESET_INDIVIDUAL flag")
                    } else {
                        format!("pool misses TRANSIENT flag")
                    }
                }
            }
        )
    }
}

#[derive(Debug)]
pub struct CmdBufferPool<B: hal::Backend> {
    device: DeviceHandle<B>,
    pool: ManuallyDrop<B::CommandPool>,
    flags: pool::CommandPoolCreateFlags,
}

impl<B: hal::Backend> CmdBufferPool<B> {
    pub fn new(
        device: DeviceHandle<B>,
        queue: &Queue<B>,
        flags: pool::CommandPoolCreateFlags,
    ) -> Result<Self, device::OutOfMemory> {
        let cmd_pool =
            ManuallyDrop::new(unsafe { device.create_command_pool(queue.family, flags)? });

        Ok(Self {
            device,
            pool: cmd_pool,
            flags,
        })
    }

    /// # Synchronization: You may NOT free the pool if a command mem is still in use (pool memory still in use)
    pub unsafe fn reset(&mut self, release_resources: bool) {
        self.pool.reset(release_resources);
    }

    pub fn allocate_one(&mut self, level: command::Level) -> B::CommandBuffer {
        unsafe { self.pool.allocate_one(level) }
    }

    pub fn allocate(&mut self, count: usize, level: command::Level) -> Vec<B::CommandBuffer> {
        let mut storage = Vec::with_capacity(count);

        unsafe {
            self.pool.allocate(count, level, &mut storage);
        }
        storage
    }

    pub fn free_single(&mut self, cmd_buffer: B::CommandBuffer) -> Result<(), CommandError> {
        if !self
            .flags
            .contains(pool::CommandPoolCreateFlags::RESET_INDIVIDUAL)
        {
            return Err(CommandError::PoolMissesCreationFlag(
                pool::CommandPoolCreateFlags::RESET_INDIVIDUAL,
            ));
        }

        unsafe { self.pool.free(std::iter::once(cmd_buffer)) }
        Ok(())
    }
}

impl<B: hal::Backend> Drop for CmdBufferPool<B> {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();
        unsafe {
            self.device
                .destroy_command_pool(ManuallyDrop::into_inner(std::ptr::read(&self.pool)));
        }
    }
}
