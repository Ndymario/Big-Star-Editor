//! The accent colour the person chose for their system, and on Linux whether it is in dark mode, which winit does not
//! say there. Each system is asked its own way, and off the interface's thread: a desktop that does not answer must not
//! hold up a frame.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long an answer stands before the system is asked again; the colour and the mode can change while the editor
/// runs, the mode on a schedule.
const FRESH: Duration = Duration::from_secs(5);

#[derive(Default)]
struct State {
    color: Option<[u8; 3]>,
    dark: Option<bool>,
    asked: Option<Instant>,
}

/// The system's accent colour and mode as last heard, asked for again in the background as they grow stale.
#[derive(Clone, Default)]
pub struct Accent(Arc<Mutex<State>>);

impl Accent {
    /// Starts asking straight away, so the answer is there by the first frame.
    pub fn watch() -> Self {
        let accent = Self::default();
        accent.color();
        accent
    }

    pub fn color(&self) -> Option<[u8; 3]> {
        self.heard(|state| state.color)
    }

    /// Whether the system is in dark mode, where it says so here rather than through winit.
    pub fn dark(&self) -> Option<bool> {
        self.heard(|state| state.dark)
    }

    fn heard<T>(&self, read: impl FnOnce(&State) -> Option<T>) -> Option<T> {
        let Ok(mut state) = self.0.lock() else { return None };
        if state.asked.is_none_or(|asked| asked.elapsed() >= FRESH) {
            state.asked = Some(Instant::now());
            let shared = self.0.clone();
            let asking = std::thread::Builder::new().name("accent".to_owned()).spawn(move || {
                let (color, dark) = (system(), dark_mode());
                if let Ok(mut state) = shared.lock() {
                    state.color = color;
                    state.dark = dark;
                    // An answer that took long is not asked for again the moment it arrives.
                    state.asked = Some(Instant::now());
                }
            });
            if let Err(error) = asking {
                log::warn!("could not ask for the accent colour: {error}");
            }
        }
        read(&state)
    }
}

/// One of the desktop portal's `org.freedesktop.appearance` settings.
#[cfg(all(unix, not(target_os = "macos")))]
fn appearance(key: &str) -> Option<zbus::zvariant::OwnedValue> {
    let connection = zbus::blocking::Connection::session().ok()?;
    let reply = connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "ReadOne",
            &("org.freedesktop.appearance", key),
        )
        .ok()?;
    reply.body().deserialize().ok()
}

/// The desktop portal's `accent-color` setting, which GNOME, KDE and the others that have one all answer.
#[cfg(all(unix, not(target_os = "macos")))]
fn system() -> Option<[u8; 3]> {
    let (red, green, blue) = <(f64, f64, f64)>::try_from(appearance("accent-color")?).ok()?;
    from_fractions([red, green, blue])
}

/// The desktop portal's `color-scheme` setting.
#[cfg(all(unix, not(target_os = "macos")))]
fn dark_mode() -> Option<bool> {
    from_color_scheme(u32::try_from(appearance("color-scheme")?).ok()?)
}

/// Windows and macOS tell winit their mode, which egui hands on.
#[cfg(not(all(unix, not(target_os = "macos"))))]
fn dark_mode() -> Option<bool> {
    None
}

/// The portal's 1 is dark and 2 light; 0, no preference, is left to the editor.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
fn from_color_scheme(scheme: u32) -> Option<bool> {
    match scheme {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

/// `reg query` rather than the registry's own calls, which need `unsafe`.
#[cfg(windows)]
fn system() -> Option<[u8; 3]> {
    use std::os::windows::process::CommandExt as _;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\DWM", "/v", "AccentColor"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    from_registry(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "macos")]
fn system() -> Option<[u8; 3]> {
    let output = std::process::Command::new("defaults").args(["read", "-g", "AppleAccentColor"]).output().ok()?;
    // No such setting is the default, "multicolour", which is blue wherever an app has no colour of its own.
    Some(from_apple(String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(4)))
}

/// Red, green and blue from 0 to 1; the portal answers with something outside that when no colour is set.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
fn from_fractions(channels: [f64; 3]) -> Option<[u8; 3]> {
    channels.iter().all(|channel| (0.0..=1.0).contains(channel)).then(|| channels.map(|channel| (channel * 255.0).round() as u8))
}

/// The colour in what `reg query … /v AccentColor` prints: a line ending in `0xAABBGGRR`.
#[cfg_attr(not(windows), allow(dead_code))]
fn from_registry(output: &str) -> Option<[u8; 3]> {
    let hex = output.split_whitespace().find_map(|word| word.strip_prefix("0x"))?;
    let [_, blue, green, red] = u32::from_str_radix(hex, 16).ok()?.to_be_bytes();
    Some([red, green, blue])
}

/// macOS keeps the accent as the number of one of its eight colours.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn from_apple(number: i32) -> [u8; 3] {
    match number {
        -1 => [152, 152, 157],
        0 => [255, 69, 58],
        1 => [255, 149, 0],
        2 => [255, 204, 0],
        3 => [52, 199, 89],
        5 => [175, 82, 222],
        6 => [255, 55, 95],
        _ => [0, 122, 255],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_portals_fractions_become_bytes_and_an_unset_colour_is_none() {
        assert_eq!(from_fractions([1.0, 0.5, 0.0]), Some([255, 128, 0]));
        assert_eq!(from_fractions([-1.0, -1.0, -1.0]), None);
    }

    #[test]
    fn the_portals_colour_scheme_is_dark_light_or_unsaid() {
        assert_eq!(from_color_scheme(1), Some(true));
        assert_eq!(from_color_scheme(2), Some(false));
        assert_eq!(from_color_scheme(0), None);
    }

    #[test]
    fn the_registrys_colour_is_read_blue_first() {
        let output = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\DWM\r\n    AccentColor    REG_DWORD    0xffd47800\r\n\r\n";
        assert_eq!(from_registry(output), Some([0x00, 0x78, 0xd4]));
        assert_eq!(from_registry("ERROR: The system was unable to find the specified registry key or value."), None);
    }

    #[test]
    fn an_unknown_apple_colour_is_the_default_blue() {
        assert_eq!(from_apple(4), from_apple(99));
        assert_ne!(from_apple(0), from_apple(4));
    }
}
