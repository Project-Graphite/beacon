use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub const VERSION: u32 = 1;
pub const HOOK_MAGIC: [u8; 4] = *b"BCN1";
pub const CONTROL_MAGIC: [u8; 4] = *b"BCC1";
pub const TOKEN_LEN: usize = 64;
pub const MAX_FRAME_LEN: usize = 16 * 1024 * 1024;
pub const HOME_ENV: &str = "BEACON_HOME";
pub const ENDPOINT_FILE: &str = "endpoint";
pub const AUTOSTART_FILE: &str = "autostart";
pub const LAUNCHING_FILE: &str = "launching";
pub const BACKGROUND_ARG: &str = "--background";
pub const DOCTOR_EVENT: &str = "BeaconDoctor";
pub const DECISION_TIMEOUT_SECS: u64 = 3600;
pub const WAITING_EVENTS: [&str; 4] = ["PermissionRequest", "Stop", "Elicitation", DOCTOR_EVENT];
pub const FORWARDED_ENV: [&str; 18] = [
    "CLAUDE_PID",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_REMOTE",
    "CLAUDE_CODE_BRIDGE_SESSION_ID",
    "CLAUDE_PROJECT_DIR",
    "TERM_PROGRAM",
    "WT_SESSION",
    "WT_PROFILE_ID",
    "VSCODE_PID",
    "VSCODE_IPC_HOOK_CLI",
    "TMUX",
    "TMUX_PANE",
    "KITTY_WINDOW_ID",
    "WEZTERM_PANE",
    "WSL_DISTRO_NAME",
];

const REPLY_NO_DECISION: u8 = 0;
const REPLY_STDOUT: u8 = 1;

pub fn is_waiting_event(event: &str) -> bool {
    WAITING_EVENTS.contains(&event)
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os(HOME_ENV)
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join("Beacon")))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub pipe_name: String,
    pub token: String,
}

impl Endpoint {
    pub fn parse(text: &str) -> Option<Endpoint> {
        let mut lines = text.lines().map(str::trim);
        let pipe_name = lines.next().filter(|name| !name.is_empty())?;
        let token = lines.next()?;
        let token_is_valid =
            token.len() == TOKEN_LEN && token.bytes().all(|byte| byte.is_ascii_hexdigit());
        token_is_valid.then(|| Endpoint {
            pipe_name: pipe_name.to_string(),
            token: token.to_string(),
        })
    }

    pub fn read(home: &Path) -> Option<Endpoint> {
        Endpoint::parse(&std::fs::read_to_string(home.join(ENDPOINT_FILE)).ok()?)
    }

    pub fn to_file_text(&self) -> String {
        format!("{}\n{}\n", self.pipe_name, self.token)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Hook,
    Control,
}

pub fn channel_for(magic: [u8; 4]) -> io::Result<Channel> {
    match magic {
        HOOK_MAGIC => Ok(Channel::Hook),
        CONTROL_MAGIC => Ok(Channel::Control),
        _ => Err(invalid("bad magic")),
    }
}

pub fn tokens_match(expected: &[u8], received: &[u8]) -> bool {
    expected.len() == received.len()
        && expected
            .iter()
            .zip(received)
            .fold(0u8, |difference, (left, right)| difference | (left ^ right))
            == 0
}

pub fn frame_len(prefix: [u8; 4]) -> io::Result<usize> {
    let len = u32::from_le_bytes(prefix) as usize;
    if len > MAX_FRAME_LEN {
        return Err(invalid("frame exceeds the maximum size"));
    }
    Ok(len)
}

pub fn read_frame<R: Read>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut prefix = [0u8; 4];
    reader.read_exact(&mut prefix)?;
    let mut bytes = vec![0u8; frame_len(prefix)?];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub fn write_frame<W: Write>(writer: &mut W, bytes: &[u8]) -> io::Result<()> {
    writer.write_all(&frame_prefix(bytes)?)?;
    writer.write_all(bytes)
}

fn frame_prefix(bytes: &[u8]) -> io::Result<[u8; 4]> {
    if bytes.len() > MAX_FRAME_LEN {
        return Err(invalid("frame exceeds the maximum size"));
    }
    Ok((bytes.len() as u32).to_le_bytes())
}

pub fn read_preamble<R: Read>(reader: &mut R) -> io::Result<(Channel, [u8; TOKEN_LEN])> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    let channel = channel_for(magic)?;
    let mut token = [0u8; TOKEN_LEN];
    reader.read_exact(&mut token)?;
    Ok((channel, token))
}

pub fn encode_preamble(channel: Channel, token: &str) -> io::Result<Vec<u8>> {
    if token.len() != TOKEN_LEN {
        return Err(invalid("token has the wrong length"));
    }
    let magic = match channel {
        Channel::Hook => HOOK_MAGIC,
        Channel::Control => CONTROL_MAGIC,
    };
    Ok([&magic[..], token.as_bytes()].concat())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRequest {
    pub meta: Vec<u8>,
    pub payload: Vec<u8>,
}

impl HookRequest {
    pub fn encode(&self, token: &str) -> io::Result<Vec<u8>> {
        let mut bytes = encode_preamble(Channel::Hook, token)?;
        write_frame(&mut bytes, &self.meta)?;
        write_frame(&mut bytes, &self.payload)?;
        Ok(bytes)
    }

    pub fn read_body<R: Read>(reader: &mut R) -> io::Result<HookRequest> {
        Ok(HookRequest {
            meta: read_frame(reader)?,
            payload: read_frame(reader)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookReply {
    NoDecision,
    Stdout(Vec<u8>),
}

impl HookReply {
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        let (kind, bytes): (u8, &[u8]) = match self {
            HookReply::NoDecision => (REPLY_NO_DECISION, &[]),
            HookReply::Stdout(bytes) => (REPLY_STDOUT, bytes),
        };
        let mut encoded = vec![kind];
        write_frame(&mut encoded, bytes)?;
        Ok(encoded)
    }

    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<HookReply> {
        let mut kind = [0u8; 1];
        reader.read_exact(&mut kind)?;
        let bytes = read_frame(reader)?;
        match kind[0] {
            REPLY_NO_DECISION => Ok(HookReply::NoDecision),
            REPLY_STDOUT => Ok(HookReply::Stdout(bytes)),
            _ => Err(invalid("unknown reply kind")),
        }
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn hook_request_round_trips() {
        let request = HookRequest {
            meta: br#"{"v":1,"event":"Stop"}"#.to_vec(),
            payload: br#"{"session_id":"s"}"#.to_vec(),
        };
        let mut reader = Cursor::new(request.encode(TOKEN).unwrap());
        let (channel, token) = read_preamble(&mut reader).unwrap();
        assert_eq!(channel, Channel::Hook);
        assert!(tokens_match(TOKEN.as_bytes(), &token));
        assert_eq!(HookRequest::read_body(&mut reader).unwrap(), request);
        assert_eq!(reader.position() as usize, reader.get_ref().len());
    }

    #[test]
    fn empty_frames_round_trip() {
        let request = HookRequest {
            meta: Vec::new(),
            payload: Vec::new(),
        };
        let mut reader = Cursor::new(request.encode(TOKEN).unwrap());
        read_preamble(&mut reader).unwrap();
        assert_eq!(HookRequest::read_body(&mut reader).unwrap(), request);
    }

    #[test]
    fn control_preamble_round_trips() {
        let mut reader = Cursor::new(encode_preamble(Channel::Control, TOKEN).unwrap());
        let (channel, token) = read_preamble(&mut reader).unwrap();
        assert_eq!(channel, Channel::Control);
        assert_eq!(&token[..], TOKEN.as_bytes());
    }

    #[test]
    fn replies_round_trip() {
        for reply in [
            HookReply::NoDecision,
            HookReply::Stdout(br#"{"beacon":"ok"}"#.to_vec()),
        ] {
            let encoded = reply.encode().unwrap();
            assert_eq!(
                HookReply::read_from(&mut Cursor::new(encoded)).unwrap(),
                reply
            );
        }
    }

    #[test]
    fn reply_wire_layout_is_kind_then_length_then_bytes() {
        assert_eq!(HookReply::NoDecision.encode().unwrap(), [0, 0, 0, 0, 0]);
        assert_eq!(
            HookReply::Stdout(b"ab".to_vec()).encode().unwrap(),
            [1, 2, 0, 0, 0, b'a', b'b']
        );
    }

    #[test]
    fn oversize_frames_are_rejected() {
        let oversize = ((MAX_FRAME_LEN + 1) as u32).to_le_bytes();
        assert_eq!(
            frame_len(oversize).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            frame_len((MAX_FRAME_LEN as u32).to_le_bytes()).unwrap(),
            MAX_FRAME_LEN
        );
        let mut reader = Cursor::new(oversize.to_vec());
        assert_eq!(
            read_frame(&mut reader).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let too_big = vec![0u8; MAX_FRAME_LEN + 1];
        assert!(write_frame(&mut Vec::new(), &too_big).is_err());
    }

    #[test]
    fn bad_magic_is_rejected() {
        let mut bytes = b"BCN2".to_vec();
        bytes.extend_from_slice(TOKEN.as_bytes());
        assert_eq!(
            read_preamble(&mut Cursor::new(bytes)).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(channel_for(*b"bcn1").is_err());
    }

    #[test]
    fn unknown_reply_kind_is_rejected() {
        assert!(HookReply::read_from(&mut Cursor::new(vec![7, 0, 0, 0, 0])).is_err());
    }

    #[test]
    fn truncated_frames_fail() {
        let mut reader = Cursor::new(vec![5, 0, 0, 0, b'a']);
        assert_eq!(
            read_frame(&mut reader).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn tokens_compare_whole_values() {
        assert!(tokens_match(b"abcd", b"abcd"));
        assert!(!tokens_match(b"abcd", b"abce"));
        assert!(!tokens_match(b"abcd", b"abc"));
    }

    #[test]
    fn endpoint_file_round_trips() {
        let endpoint = Endpoint {
            pipe_name: r"\\.\pipe\beacon-0123456789abcdef".to_string(),
            token: TOKEN.to_string(),
        };
        assert_eq!(
            Endpoint::parse(&endpoint.to_file_text()),
            Some(endpoint.clone())
        );
        assert_eq!(
            Endpoint::parse(&endpoint.to_file_text().replace('\n', "\r\n")),
            Some(endpoint)
        );
    }

    #[test]
    fn malformed_endpoint_files_are_rejected() {
        assert_eq!(Endpoint::parse(""), None);
        assert_eq!(Endpoint::parse(r"\\.\pipe\x"), None);
        assert_eq!(Endpoint::parse(&format!("\n{TOKEN}")), None);
        assert_eq!(
            Endpoint::parse(&format!(r"\\.\pipe\x{}{}", "\n", &TOKEN[1..])),
            None
        );
        assert_eq!(
            Endpoint::parse(&format!(r"\\.\pipe\x{}{}", "\n", TOKEN.replace('a', "z"))),
            None
        );
    }

    #[test]
    fn waiting_events_are_recognised() {
        for event in ["PermissionRequest", "Stop", "Elicitation", "BeaconDoctor"] {
            assert!(is_waiting_event(event));
        }
        assert!(!is_waiting_event("PreToolUse"));
    }
}
