//! The accent colour the person chose for their system, and on Linux whether it is in dark mode, which winit does not
//! say there. Each system is asked its own way, and off the interface's thread: a desktop that does not answer must not
//! hold up a frame. On Linux the desktop portal says when either changes; elsewhere the colour is asked for again.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long an answer stands before the system is asked again, where it does not say when the colour changes.
const FRESH: Duration = Duration::from_secs(5);

#[derive(Default)]
struct State {
    color: Option<[u8; 3]>,
    dark: Option<bool>,
    asked: Option<Instant>,
    /// The system says when either changes, so there is no asking again.
    told: bool,
    /// Brings the window round to a change it was told of.
    #[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// The system's accent colour and mode as last heard, kept up to date in the background.
#[derive(Clone, Default)]
pub struct Accent(Arc<Mutex<State>>);

impl Accent {
    /// Starts asking straight away, so the answer is there by the first frame, and listens for changes where the system
    /// says them.
    pub fn watch() -> Self {
        let accent = Self::default();
        accent.color();
        accent.listen();
        accent
    }

    /// Has `wake` called when the colour or the mode changes, so the window redraws without waiting for input.
    pub fn wake_with(&self, wake: impl Fn() + Send + Sync + 'static) {
        if let Ok(mut state) = self.0.lock() {
            state.wake = Some(Arc::new(wake));
        }
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
        if !state.told && state.asked.is_none_or(|asked| asked.elapsed() >= FRESH) {
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

    /// Listens for the portal's `SettingChanged`, which comes the moment the person, or a schedule, changes the colour
    /// or the mode. A desktop without the portal is asked every [`FRESH`] instead.
    #[cfg(all(unix, not(target_os = "macos")))]
    fn listen(&self) {
        let shared = self.0.clone();
        let listening = std::thread::Builder::new().name("appearance".to_owned()).spawn(move || {
            if let Err(error) = listen(&shared) {
                log::info!("the desktop portal does not say when the appearance changes: {error}");
            }
            if let Ok(mut state) = shared.lock() {
                state.told = false;
            }
        });
        if let Err(error) = listening {
            log::warn!("could not listen for appearance changes: {error}");
        }
    }

    #[cfg(not(all(unix, not(target_os = "macos"))))]
    fn listen(&self) {}
}

#[cfg(all(unix, not(target_os = "macos")))]
const PORTAL: &str = "org.freedesktop.portal.Desktop";
#[cfg(all(unix, not(target_os = "macos")))]
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
#[cfg(all(unix, not(target_os = "macos")))]
const SETTINGS: &str = "org.freedesktop.portal.Settings";
#[cfg(all(unix, not(target_os = "macos")))]
const APPEARANCE: &str = "org.freedesktop.appearance";

/// Subscribes, then reads what is set now, so no change falls between the two; then takes each change as it comes.
#[cfg(all(unix, not(target_os = "macos")))]
fn listen(shared: &Mutex<State>) -> zbus::Result<()> {
    let connection = zbus::blocking::Connection::session()?;
    let proxy = zbus::blocking::Proxy::new(&connection, PORTAL, PORTAL_PATH, SETTINGS)?;
    let changes = proxy.receive_signal("SettingChanged")?;
    let (color, dark) = (system(), dark_mode());
    if let Ok(mut state) = shared.lock() {
        (state.color, state.dark, state.told) = (color, dark, true);
    }
    for message in changes {
        let Ok((namespace, key, value)) = message.body().deserialize::<(String, String, zbus::zvariant::OwnedValue)>() else {
            continue;
        };
        if namespace != APPEARANCE {
            continue;
        }
        let Ok(mut state) = shared.lock() else { break };
        match key.as_str() {
            "accent-color" => state.color = accent_from(value),
            "color-scheme" => state.dark = dark_from(value),
            _ => continue,
        }
        if let Some(wake) = &state.wake {
            wake();
        }
    }
    Err(zbus::Error::Failure("the portal's signals stopped".to_owned()))
}

/// One of the desktop portal's `org.freedesktop.appearance` settings.
#[cfg(all(unix, not(target_os = "macos")))]
fn appearance(key: &str) -> Option<zbus::zvariant::OwnedValue> {
    let connection = zbus::blocking::Connection::session().ok()?;
    let reply = connection.call_method(Some(PORTAL), PORTAL_PATH, Some(SETTINGS), "ReadOne", &(APPEARANCE, key)).ok()?;
    reply.body().deserialize().ok()
}

/// The desktop portal's `accent-color` setting, which GNOME, KDE and the others that have one all answer.
#[cfg(all(unix, not(target_os = "macos")))]
fn system() -> Option<[u8; 3]> {
    accent_from(appearance("accent-color")?)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn accent_from(value: zbus::zvariant::OwnedValue) -> Option<[u8; 3]> {
    let (red, green, blue) = <(f64, f64, f64)>::try_from(value).ok()?;
    from_fractions([red, green, blue])
}

/// The desktop portal's `color-scheme` setting.
#[cfg(all(unix, not(target_os = "macos")))]
fn dark_mode() -> Option<bool> {
    dark_from(appearance("color-scheme")?)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn dark_from(value: zbus::zvariant::OwnedValue) -> Option<bool> {
    from_color_scheme(u32::try_from(value).ok()?)
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
