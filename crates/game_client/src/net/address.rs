//! Server address parsing and text-field filtering. Pure, so the lobby rules are unit-tested.

pub const DEFAULT_PORT: u16 = 7777;
pub const WS_PATH: &str = "/ws";

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AddrError {
    Empty,
    BadScheme,
    BadHost,
    BadPort,
    /// `ws://` requested from an `https://` page.
    MixedContent,
}

impl AddrError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Empty => "Enter a server address first.",
            Self::BadScheme => "Use ws://, wss://, http:// or https:// (or just the host).",
            Self::BadHost => "That does not look like a host name or IP address.",
            Self::BadPort => "Port must be a number from 1 to 65535.",
            Self::MixedContent => {
                "Browsers block ws:// from an https page. Use wss://, or open the game from http://<server>:7777/"
            }
        }
    }
}

/// Turns what the player typed into a websocket URL. `page_is_https` is true only for the web
/// build served over https, where plain `ws://` cannot work.
pub fn normalize(input: &str, page_is_https: bool) -> Result<String, AddrError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(AddrError::Empty);
    }
    let (secure, explicit_scheme, rest) = match input.split_once("://") {
        Some((scheme, rest)) => match scheme.to_ascii_lowercase().as_str() {
            "ws" | "http" => (false, true, rest),
            "wss" | "https" => (true, true, rest),
            _ => return Err(AddrError::BadScheme),
        },
        None => (false, false, input),
    };
    // A path is ignored: the endpoint is always /ws.
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let (host, port) = split_host_port(authority)?;
    if page_is_https && !secure {
        return Err(AddrError::MixedContent);
    }
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    let scheme = if secure { "wss" } else { "ws" };
    // wss://host is normally a reverse proxy on 443, so only plain ws gets the game port by default.
    let port = match port {
        Some(p) => format!(":{p}"),
        None if secure && explicit_scheme => String::new(),
        None => format!(":{DEFAULT_PORT}"),
    };
    Ok(format!("{scheme}://{host}{port}{WS_PATH}"))
}

fn split_host_port(authority: &str) -> Result<(&str, Option<u16>), AddrError> {
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let (host, tail) = rest.split_once(']').ok_or(AddrError::BadHost)?;
        if host.is_empty()
            || !host
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
        {
            return Err(AddrError::BadHost);
        }
        match tail {
            "" => (host, None),
            t => (host, Some(t.strip_prefix(':').ok_or(AddrError::BadHost)?)),
        }
    } else {
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (authority, None),
        };
        let valid = !host.is_empty()
            && !host.starts_with(['.', '-'])
            && !host.ends_with(['.', '-'])
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
        if !valid {
            return Err(AddrError::BadHost);
        }
        (host, port)
    };
    let port = match port {
        None => None,
        Some(p) => match p.parse::<u16>() {
            Ok(n) if n > 0 => Some(n),
            _ => return Err(AddrError::BadPort),
        },
    };
    Ok((host, port))
}

/// The "This server" shortcut: the host that served the web page. Not offered on github.io,
/// where the page cannot be a game server.
pub fn this_server_url(protocol: &str, host: &str) -> Option<String> {
    if host.is_empty() || host.ends_with("github.io") {
        return None;
    }
    let scheme = if protocol.starts_with("https") {
        "wss"
    } else {
        "ws"
    };
    Some(format!("{scheme}://{host}{WS_PATH}"))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Address,
    Name,
    Room,
    /// Optional title of a room the player creates (`net_proto::sanitize_title` rules).
    Title,
}

impl Field {
    pub fn max_len(self) -> usize {
        match self {
            Field::Address => 100,
            Field::Name => 16,
            // Longest code: QUARTERLY-PARADIGM-XXXXXXXX (27); a little slack for typos.
            Field::Room => 32,
            Field::Title => net_proto::MAX_TITLE_CHARS,
        }
    }

    /// The character to store for a typed one, or None to drop it.
    pub fn filter(self, c: char) -> Option<char> {
        match self {
            Field::Address => (c.is_ascii_alphanumeric() || ":.-_/[]".contains(c)).then_some(c),
            Field::Name | Field::Title => {
                (c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-')).then_some(c)
            }
            // Codes read aloud get typed with spaces; they separate words like dashes.
            Field::Room => match c {
                ' ' | '-' => Some('-'),
                c if c.is_ascii_alphanumeric() => Some(c.to_ascii_uppercase()),
                _ => None,
            },
        }
    }

    /// Appends the accepted characters of `text` up to the field limit.
    pub fn push_str(self, target: &mut String, text: &str) {
        for c in text.chars().filter_map(|c| self.filter(c)) {
            if target.chars().count() >= self.max_len() {
                break;
            }
            target.push(c);
        }
    }

    /// Re-filters a whole value (hidden HTML input, pasted text).
    pub fn sanitize(self, text: &str) -> String {
        let mut out = String::new();
        self.push_str(&mut out, text);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_common_inputs() {
        let cases = [
            ("192.168.1.20", "ws://192.168.1.20:7777/ws"),
            ("192.168.1.20:9000", "ws://192.168.1.20:9000/ws"),
            ("  myhost ", "ws://myhost:7777/ws"),
            ("ws://myhost", "ws://myhost:7777/ws"),
            ("ws://myhost:1234/whatever", "ws://myhost:1234/ws"),
            ("WSS://Example.com", "wss://Example.com/ws"),
            ("wss://example.com:8443/", "wss://example.com:8443/ws"),
            ("http://lan-box:7777/", "ws://lan-box:7777/ws"),
            ("https://game.example.com/", "wss://game.example.com/ws"),
            ("[::1]:7777", "ws://[::1]:7777/ws"),
            ("[::1]", "ws://[::1]:7777/ws"),
        ];
        for (input, want) in cases {
            assert_eq!(normalize(input, false).as_deref(), Ok(want), "{input}");
        }
    }

    #[test]
    fn rejects_bad_input() {
        let cases = [
            ("", AddrError::Empty),
            ("   ", AddrError::Empty),
            ("ftp://x", AddrError::BadScheme),
            ("bad host", AddrError::BadHost),
            ("-host", AddrError::BadHost),
            ("user@host", AddrError::BadHost),
            ("[::1", AddrError::BadHost),
            ("host:0", AddrError::BadPort),
            ("host:70000", AddrError::BadPort),
            ("host:abc", AddrError::BadPort),
            ("host:", AddrError::BadPort),
        ];
        for (input, want) in cases {
            assert_eq!(normalize(input, false), Err(want), "{input}");
        }
    }

    #[test]
    fn https_page_blocks_plain_ws_only() {
        assert_eq!(
            normalize("192.168.1.20", true),
            Err(AddrError::MixedContent)
        );
        assert_eq!(normalize("ws://a", true), Err(AddrError::MixedContent));
        assert_eq!(normalize("http://a", true), Err(AddrError::MixedContent));
        assert_eq!(normalize("wss://a", true).as_deref(), Ok("wss://a/ws"));
    }

    #[test]
    fn this_server_skips_github_pages() {
        assert_eq!(this_server_url("https:", "me.github.io"), None);
        assert_eq!(
            this_server_url("http:", "10.0.0.5:7777").as_deref(),
            Some("ws://10.0.0.5:7777/ws")
        );
        assert_eq!(
            this_server_url("https:", "g.example.com").as_deref(),
            Some("wss://g.example.com/ws")
        );
        assert_eq!(this_server_url("http:", ""), None);
    }

    #[test]
    fn field_filtering() {
        let mut s = String::new();
        Field::Name.push_str(&mut s, "Bob <script>\n_1-x");
        assert_eq!(s, "Bob script_1-x");
        assert_eq!(Field::Room.sanitize("ab-12 zz!"), "AB-12-ZZ");
        let code = "quarterly paradigm-7k3m9qza";
        let typed = Field::Room.sanitize(code);
        assert_eq!(typed, "QUARTERLY-PARADIGM-7K3M9QZA");
        assert_eq!(
            net_proto::parse_room_code(&typed).as_deref(),
            Ok("QUARTERLY-PARADIGM-7K3M9QZA")
        );
        assert_eq!(
            Field::Address.sanitize("ws://10.0.0.1:7777 "),
            "ws://10.0.0.1:7777"
        );
        assert_eq!(
            Field::Room.sanitize(&"a".repeat(50)).len(),
            Field::Room.max_len()
        );
    }
}
