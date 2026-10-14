//! The window's icon on Wayland. winit only gives a window its icon on X11
//! and Windows; on Wayland the compositor looks for a desktop entry named by
//! the app id, which a build run from the source tree or an AppImage that was
//! never integrated does not have. Compositors with `xdg-toplevel-icon-v1`
//! (KWin, Mutter, ...) take the pixels straight from the app instead.
//!
//! Temporary: winit does this itself from 0.31 (rust-windowing/winit#4288,
//! merged to master in June 2025 and never backported to 0.30), but eframe
//! 0.36 is on winit 0.30. Once eframe moves to winit 0.31, the `with_icon` in
//! main.rs covers Wayland: delete this module, its call in `main`, the
//! Linux-only dependencies in Cargo.toml (winit, wayland-client,
//! wayland-protocols, rustix) and put `unsafe_code` back to "forbid" in the
//! workspace Cargo.toml.

use std::io::Write as _;
use std::os::fd::AsFd as _;

use eframe::egui;
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_buffer, wl_registry, wl_shm, wl_shm_pool};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop};
use wayland_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;
use wayland_protocols::xdg::toplevel_icon::v1::client::{
    xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1, xdg_toplevel_icon_v1::XdgToplevelIconV1,
};
use winit::platform::wayland::WindowExtWayland as _;
use winit::raw_window_handle::{HasDisplayHandle as _, RawDisplayHandle};

struct State;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: wl_shm_pool::WlShmPool);
delegate_noop!(State: ignore wl_buffer::WlBuffer);
delegate_noop!(State: ignore XdgToplevelIconManagerV1);
delegate_noop!(State: XdgToplevelIconV1);

/// Gives the window `icon` when it is a Wayland window and the compositor
/// takes icons from apps; does nothing otherwise. Takes effect with the
/// window's next frame.
pub fn set(window: &winit::window::Window, icon: &egui::IconData) {
    if let Err(e) = try_set(window, icon) {
        log::debug!("the window keeps the compositor's icon: {e}");
    }
}

#[allow(unsafe_code)]
fn try_set(window: &winit::window::Window, icon: &egui::IconData) -> Result<(), String> {
    let Some(toplevel) = window.xdg_toplevel() else {
        return Ok(());
    };
    let RawDisplayHandle::Wayland(display) =
        window.display_handle().map_err(|e| e.to_string())?.as_raw()
    else {
        return Ok(());
    };
    let side = icon.width as i32;
    if icon.width != icon.height || side == 0 {
        return Err("the icon is not square".into());
    }

    // SAFETY: the display and the toplevel belong to the window, which
    // outlives this connection (both last until the process ends).
    let conn = Connection::from_backend(unsafe {
        Backend::from_foreign_display(display.display.as_ptr().cast())
    });
    let toplevel =
        unsafe { ObjectId::from_ptr(XdgToplevel::interface(), toplevel.as_ptr().cast()) }
            .map_err(|e| e.to_string())?;
    let toplevel = XdgToplevel::from_id(&conn, toplevel).map_err(|e| e.to_string())?;

    let (globals, mut queue): (_, EventQueue<State>) =
        registry_queue_init(&conn).map_err(|e| e.to_string())?;
    let qh = queue.handle();
    let manager: XdgToplevelIconManagerV1 = globals
        .bind(&qh, 1..=1, ())
        .map_err(|_| "the compositor has no toplevel icons")?;
    let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ()).map_err(|e| e.to_string())?;

    // wl_shm's ARGB8888: premultiplied, little-endian, so B G R A in memory.
    let pixels: Vec<u8> = icon
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[r, g, b, a]| {
            let mul = |c: u8| ((u16::from(c) * u16::from(a) + 127) / 255) as u8;
            [mul(b), mul(g), mul(r), a]
        })
        .collect();
    let fd = rustix::fs::memfd_create("bigstar-icon", rustix::fs::MemfdFlags::CLOEXEC)
        .map_err(|e| e.to_string())?;
    let mut file = std::fs::File::from(fd);
    file.write_all(&pixels).map_err(|e| e.to_string())?;

    let pool = shm.create_pool(file.as_fd(), pixels.len() as i32, &qh, ());
    let buffer = pool.create_buffer(0, side, side, side * 4, wl_shm::Format::Argb8888, &qh, ());
    let toplevel_icon = manager.create_icon(&qh, ());
    toplevel_icon.add_buffer(&buffer, 1);
    manager.set_icon(&toplevel, Some(&toplevel_icon));
    queue.roundtrip(&mut State).map_err(|e| e.to_string())?;

    // The buffer has to outlive the icon, and the icon stays set on the
    // window for as long as it is open: the whole run. So everything here
    // is kept, the queue too, so that nothing of it is left to winit's.
    std::mem::forget((queue, conn, buffer, pool, toplevel_icon, manager, shm));
    Ok(())
}
