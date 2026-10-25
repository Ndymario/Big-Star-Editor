//! ROMs a connection uploads. Each one gets a host of its own -- its own open ROM, levels, graphics and events -- that
//! no other connection can reach, and that has no workspace, so nothing in it can name a file on the host's disk. It
//! lives as long as the connection: the host writes none of it anywhere, and what leaves is the download.

use std::sync::atomic::Ordering;

use bigstar_protocol::ROM_CHUNK_BYTES;

use super::*;

pub(super) const GUEST_REFUSAL: &str = "This connection can only edit a ROM it uploads.";

#[derive(Default)]
pub(super) struct Uploads {
    receiving: Option<Receiving>,
    open: Option<UploadedRom>,
    /// Events an uploaded ROM sent before it was put away, not drained yet.
    carried: Vec<Event>,
}

struct Receiving {
    name: String,
    total: usize,
    bytes: Vec<u8>,
    slot: Slot,
}

struct UploadedRom {
    shared: Arc<Shared>,
    events: Receiver<Event>,
    _slot: Slot,
}

/// One of the host's places for an uploaded ROM, given back when dropped.
struct Slot(Arc<Shared>);

impl Slot {
    fn take(host: &Arc<Shared>) -> Result<Slot> {
        let mut held = host.uploads.load(Ordering::SeqCst);
        loop {
            if held >= host.options.max_uploaded_roms {
                return Err(Error::new(ErrorCode::Unsupported, "The host is holding as many uploaded ROMs as it can. Try again later."));
            }
            match host.uploads.compare_exchange(held, held + 1, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return Ok(Slot(Arc::clone(host))),
                Err(now) => held = now,
            }
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.uploads.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Uploads {
    pub fn drain_events(&mut self) -> Vec<Event> {
        let mut events = std::mem::take(&mut self.carried);
        if let Some(open) = &self.open {
            events.extend(open.events.try_iter());
        }
        events
    }

    /// Puts the uploaded ROM away. Its last events are passed on when the frontend closed it, and dropped when
    /// something replaced it: the answer to whatever replaced it is all the frontend needs, as when a host opens one
    /// project over another.
    fn put_away(&mut self, pass_events_on: bool) {
        if let Some(open) = self.open.take()
            && pass_events_on
        {
            self.carried.extend(open.events.try_iter());
        }
    }
}

/// Requests that name something in the workspace, which an uploaded ROM's own host has none of.
fn addresses_workspace(request: &Request) -> bool {
    matches!(
        request,
        Request::ListProjects | Request::BrowseDirectory { .. } | Request::OpenProject { .. } | Request::SetCleanRom { .. } | Request::OpenRom { .. }
    )
}

impl HostSession {
    /// Sends a request where it belongs: to this connection's uploaded ROM when it is about that, otherwise to the
    /// workspace, which a guest never reaches.
    pub(super) fn route(&mut self, request: Request) -> Result<Response> {
        if let Request::UploadRom { name, offset, total, bytes } = request {
            return self.receive(name, offset, total, bytes);
        }
        if let Some(open) = &self.uploads.open {
            if !addresses_workspace(&request) {
                let closing = matches!(request, Request::CloseProject);
                let response = Shared::handle(&open.shared, request)?;
                if closing {
                    self.uploads.put_away(true);
                }
                return Ok(response);
            }
            if self.guest {
                return Err(invalid(GUEST_REFUSAL));
            }
            // Opening something else puts the uploaded ROM away once it has opened, as opening a project closes the one
            // before.
            if matches!(request, Request::OpenProject { .. } | Request::OpenRom { .. }) {
                if open.shared.has_unsaved_levels() {
                    return Err(invalid("save or close the open levels before opening another project"));
                }
                let response = Shared::handle(&self.shared, request)?;
                self.uploads.put_away(false);
                return Ok(response);
            }
        } else if self.guest {
            return Err(invalid(GUEST_REFUSAL));
        }
        Shared::handle(&self.shared, request)
    }

    fn receive(&mut self, name: String, offset: u64, total: u64, bytes: Vec<u8>) -> Result<Response> {
        if offset == 0 {
            // A new upload replaces one left unfinished.
            self.uploads.receiving = None;
            let limit = self.shared.options.max_rom_bytes;
            let total = usize::try_from(total)
                .ok()
                .filter(|total| (1..=limit).contains(total))
                .ok_or_else(|| invalid(format!("This host takes ROMs of up to {} MiB.", limit / (1024 * 1024))))?;
            if self.uploads.open.as_ref().is_some_and(|open| open.shared.has_unsaved_levels()) {
                return Err(invalid("Save or close the open levels before uploading another ROM."));
            }
            let slot = Slot::take(&self.shared)?;
            self.uploads.receiving = Some(Receiving { name: name.clone(), total, bytes: Vec::with_capacity(total), slot });
        }
        let receiving = self.uploads.receiving.as_mut().ok_or_else(|| invalid("an upload starts at offset 0"))?;
        let in_order = usize::try_from(offset).is_ok_and(|offset| offset == receiving.bytes.len())
            && usize::try_from(total).is_ok_and(|total| total == receiving.total)
            && name == receiving.name
            && bytes.len() <= ROM_CHUNK_BYTES
            && receiving.bytes.len() + bytes.len() <= receiving.total;
        if !in_order {
            self.uploads.receiving = None;
            return Err(invalid("The upload arrived out of order or larger than expected. Try again."));
        }
        receiving.bytes.extend_from_slice(&bytes);
        if receiving.bytes.len() < receiving.total {
            return Ok(Response::Uploaded { received: receiving.bytes.len() as u64 });
        }
        let Some(Receiving { name, bytes, slot, .. }) = self.uploads.receiving.take() else {
            return Err(invalid("an upload starts at offset 0"));
        };
        let shared = Shared::new(None, false, self.shared.options.clone());
        let events = shared.subscribe();
        let response = shared.open_uploaded(&name, bytes)?;
        self.uploads.put_away(false);
        self.uploads.open = Some(UploadedRom { shared, events, _slot: slot });
        Ok(response)
    }
}
