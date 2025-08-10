use std::{
    error::Error,
    sync::{mpsc, Arc},
};

use nix::sys::{
    epoll::{Epoll, EpollEvent, EpollFlags},
    eventfd::{self, EfdFlags, EventFd},
};

#[derive(Clone)]
pub struct MqSender<T> {
    sender: mpsc::Sender<T>,
    eventfd: Arc<eventfd::EventFd>,
}

pub struct MqReceiver<T> {
    receiver: mpsc::Receiver<T>,
    eventfd: Arc<eventfd::EventFd>,
}

pub fn new<T: Clone>(
    epoll: &Epoll,
    queue_id: u64,
) -> Result<(MqSender<T>, MqReceiver<T>), Box<dyn Error>> {
    let (sender, receiver) = mpsc::channel::<T>();

    let eventfd = EventFd::from_flags(EfdFlags::EFD_SEMAPHORE)?;
    epoll.add(&eventfd, EpollEvent::new(EpollFlags::EPOLLIN, queue_id))?;

    let eventfd = Arc::new(eventfd);

    let message_queue_sender = MqSender {
        sender,
        eventfd: eventfd.clone(),
    };

    let message_queue_receiver = MqReceiver {
        receiver,
        eventfd: eventfd.clone(),
    };
    Ok((message_queue_sender, message_queue_receiver))
}

impl<'a, T: 'a + Clone> MqSender<T> {
    pub fn send(&self, payload: T) -> Result<(), Box<dyn Error + 'a>> {
        self.sender.send(payload)?;
        self.eventfd.write(1)?;
        Ok(())
    }
}

impl<T: Clone> MqReceiver<T> {
    pub fn recv(&self) -> Result<T, Box<dyn Error>> {
        self.eventfd.read()?;
        Ok(self.receiver.recv()?)
    }
}

#[repr(u64)]
pub enum EventKind {
    Unknown,
    Wayland,
    Mq,
}

impl From<u64> for EventKind {
    fn from(value: u64) -> Self {
        match value {
            value if value == Self::Wayland as u64 => Self::Wayland,
            value if value == Self::Mq as u64 => Self::Mq,
            _ => Self::Unknown,
        }
    }
}
