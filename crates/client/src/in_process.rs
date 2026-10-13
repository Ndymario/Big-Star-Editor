use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use bigstar_protocol::{Request, RequestId, Service};

use crate::{Connection, ConnectionState, HostInfo, Incoming, Waker};

/// How long the worker waits for a request before it looks for events other connections produced.
const EVENT_INTERVAL: Duration = Duration::from_millis(50);

/// A host in this process, on a worker thread of its own. It speaks the same messages as a remote one, so the
/// frontend cannot tell the difference.
pub struct InProcess {
    requests: Option<Sender<(RequestId, Request)>>,
    incoming: Receiver<Incoming>,
    state: ConnectionState,
    next_id: RequestId,
    worker: Option<JoinHandle<()>>,
    pending: Vec<Incoming>,
}

impl InProcess {
    pub fn spawn(mut service: impl Service, info: HostInfo, waker: Option<Waker>) -> Self {
        let (request_sender, request_receiver) = channel::<(RequestId, Request)>();
        let (incoming_sender, incoming) = channel();
        let worker = std::thread::spawn(move || {
            let wake = || {
                if let Some(waker) = &waker {
                    waker();
                }
            };
            loop {
                let mut sent = false;
                match request_receiver.recv_timeout(EVENT_INTERVAL) {
                    Ok((id, request)) => {
                        sent = incoming_sender.send(Incoming::Response { id, result: service.handle(request) }).is_ok();
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
                for event in service.drain_events() {
                    sent |= incoming_sender.send(Incoming::Event(event)).is_ok();
                }
                if sent {
                    wake();
                }
            }
        });
        Self {
            requests: Some(request_sender),
            incoming,
            state: ConnectionState::Connected(info),
            next_id: 1,
            worker: Some(worker),
            pending: Vec::new(),
        }
    }
}

impl Connection for InProcess {
    fn send(&mut self, request: Request) -> RequestId {
        let id = self.next_id;
        self.next_id += 1;
        let delivered = self.requests.as_ref().is_some_and(|requests| requests.send((id, request)).is_ok());
        if !delivered {
            let error = bigstar_core::Error::new(bigstar_core::ErrorCode::IoError, "the host has stopped");
            self.pending.push(Incoming::Response { id, result: Err(error) });
        }
        id
    }

    fn poll(&mut self) -> Vec<Incoming> {
        let mut out = std::mem::take(&mut self.pending);
        loop {
            match self.incoming.try_recv() {
                Ok(message) => out.push(message),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !matches!(self.state, ConnectionState::Closed(_)) {
                        self.state = ConnectionState::Closed("the host has stopped".to_owned());
                        out.push(Incoming::State(self.state.clone()));
                    }
                    break;
                }
            }
        }
        out
    }

    fn state(&self) -> &ConnectionState {
        &self.state
    }
}

impl Drop for InProcess {
    fn drop(&mut self) {
        // Closing the channel ends the worker's loop.
        self.requests = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
