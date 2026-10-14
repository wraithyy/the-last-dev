//! Remembered address and display name, plus facts about the hosting page. Every failure is ignored:
//! storage can be blocked (private window) or read-only, and none of it is worth interrupting play.

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Saved {
    pub address: String,
    pub name: String,
}

// The native file format; the web build keeps fields in localStorage instead.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn encode(s: &Saved) -> String {
    format!("{}\n{}\n", s.address, s.name)
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn decode(text: &str) -> Saved {
    let mut lines = text.lines();
    Saved {
        address: lines.next().unwrap_or_default().to_string(),
        name: lines.next().unwrap_or_default().to_string(),
    }
}

/// Where the web page came from; None on native.
pub struct Page {
    pub protocol: String,
    pub host: String,
}

impl Page {
    pub fn https(&self) -> bool {
        self.protocol.starts_with("https")
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use super::{Page, Saved, decode, encode};
    use std::path::PathBuf;

    fn path() -> Option<PathBuf> {
        let env = |k| std::env::var_os(k).map(PathBuf::from);
        let base = if cfg!(target_os = "macos") {
            env("HOME")?.join("Library/Application Support")
        } else if cfg!(windows) {
            env("APPDATA")?
        } else {
            env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))?
        };
        Some(base.join("the-last-dev").join("coop.txt"))
    }

    pub fn load() -> Saved {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| decode(&t))
            .unwrap_or_default()
    }

    pub fn save(s: &Saved) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, encode(s));
    }

    pub fn page() -> Option<Page> {
        None
    }

    pub fn load_hud() -> Option<u8> {
        let p = path()?.with_file_name("hud.txt");
        std::fs::read_to_string(p).ok()?.trim().parse().ok()
    }

    pub fn save_hud(bits: u8) {
        let Some(p) = path().map(|p| p.with_file_name("hud.txt")) else {
            return;
        };
        let _ = std::fs::write(p, bits.to_string());
    }

    pub fn load_class() -> Option<String> {
        let p = path()?.with_file_name("class.txt");
        let s = std::fs::read_to_string(p).ok()?;
        Some(s.trim().to_string()).filter(|s| !s.is_empty())
    }

    pub fn save_class(class: &str) {
        let Some(p) = path().map(|p| p.with_file_name("class.txt")) else {
            return;
        };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, class);
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::{Page, Saved};

    const ADDRESS: &str = "tld.coop.address";
    const NAME: &str = "tld.coop.name";

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    pub fn load() -> Saved {
        let Some(s) = storage() else {
            return Saved::default();
        };
        let get = |k| s.get_item(k).ok().flatten().unwrap_or_default();
        Saved {
            address: get(ADDRESS),
            name: get(NAME),
        }
    }

    pub fn save(saved: &Saved) {
        if let Some(s) = storage() {
            let _ = s.set_item(ADDRESS, &saved.address);
            let _ = s.set_item(NAME, &saved.name);
        }
    }

    const HUD: &str = "tld.hud.windows";

    pub fn load_hud() -> Option<u8> {
        storage()?.get_item(HUD).ok().flatten()?.parse().ok()
    }

    pub fn save_hud(bits: u8) {
        if let Some(s) = storage() {
            let _ = s.set_item(HUD, &bits.to_string());
        }
    }

    const CLASS: &str = "tld.class";

    pub fn load_class() -> Option<String> {
        storage()?
            .get_item(CLASS)
            .ok()
            .flatten()
            .filter(|s| !s.is_empty())
    }

    pub fn save_class(class: &str) {
        if let Some(s) = storage() {
            let _ = s.set_item(CLASS, class);
        }
    }

    pub fn page() -> Option<Page> {
        let loc = web_sys::window()?.location();
        Some(Page {
            protocol: loc.protocol().ok()?,
            host: loc.host().ok()?,
        })
    }
}

pub use platform::{load, load_class, load_hud, page, save, save_class, save_hud};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_garbage() {
        let s = Saved {
            address: "10.0.0.2:7777".into(),
            name: "Bob".into(),
        };
        assert_eq!(decode(&encode(&s)), s);
        assert_eq!(decode(""), Saved::default());
        assert_eq!(decode("only-address").name, "");
    }

    #[test]
    fn https_detection() {
        let p = |proto: &str| Page {
            protocol: proto.into(),
            host: "h".into(),
        };
        assert!(p("https:").https());
        assert!(!p("http:").https());
    }
}
