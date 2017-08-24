use std::io;
use std::time::SystemTime;
use std::ascii::AsciiExt;
use std::fs::{File};
use std::path::Path;
use std::ffi::OsString;

use accept_encoding::{AcceptEncodingParser, Iter as EncodingIter};
use range::{Range, RangeParser};
use etag::Etag;
use {AcceptEncoding, Output};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Head,
    Get,
    InvalidMethod,
    InvalidRange,
}

#[derive(Debug, Clone)]
pub struct Input {
    pub(crate) mode: Mode,
    pub(crate) accept_encoding: AcceptEncoding,
    pub(crate) range: Option<Range>,
    pub(crate) if_range: Option<Result<SystemTime, Etag>>,
    pub(crate) if_match: Vec<Etag>,
    pub(crate) if_none: Vec<Etag>,
    pub(crate) if_unmodified: Option<SystemTime>,
    pub(crate) if_modified: Option<SystemTime>,
}

impl Input {
    pub fn from_headers<'x, I>(method: &str, headers: I) -> Input
        where I: Iterator<Item=(&'x str, &'x[u8])>
    {
        let mode = match method {
            "HEAD" => Mode::Head,
            "GET" => Mode::Get,
            _ => return Input {
                mode: Mode::InvalidMethod,
                accept_encoding: AcceptEncoding::identity(),
                range: None,
                if_range: None,
                if_match: Vec::new(),
                if_none: Vec::new(),
                if_unmodified: None,
                if_modified: None,
            },
        };
        let mut ae_parser = AcceptEncodingParser::new();
        let mut range_parser = RangeParser::new();
        for (key, val) in headers {
            if key.eq_ignore_ascii_case("accept-encoding") {
                ae_parser.add_header(val);
            } else if key.eq_ignore_ascii_case("range") {
                range_parser.add_header(val);
            }
        }
        let range = match range_parser.done() {
            Ok(range) => range,
            Err(()) => return Input {
                mode: Mode::InvalidRange,
                accept_encoding: AcceptEncoding::identity(),
                range: None,
                if_range: None,
                if_match: Vec::new(),
                if_none: Vec::new(),
                if_unmodified: None,
                if_modified: None,
            },
        };
        Input {
            mode: mode,
            accept_encoding: ae_parser.done(),
            range: range,
            if_range: None,
            if_match: Vec::new(),
            if_none: Vec::new(),
            if_unmodified: None,
            if_modified: None,
        }
    }
    pub fn encodings(&self) -> EncodingIter {
        self.accept_encoding.iter()
    }
    /// Open files from filesystem
    ///
    /// **Must be run in disk thread**
    pub fn file_at<P: AsRef<Path>>(&self, path: P) -> Option<Output> {
        println!("Mode {:?}", self.mode);
        let path = path.as_ref().as_os_str();
        let mut buf = OsString::with_capacity(path.len() + 3);
        for enc in self.encodings() {
            buf.clear();
            buf.push(path);
            buf.push(enc.suffix());
            let path = Path::new(&buf);
            match File::open(path).and_then(|f| f.metadata().map(|m| (f, m))) {
                Ok((f, meta)) => {
                    let outp = Output::from_file(self, enc, &meta, f);
                    return Some(outp);
                }
                Err(e) => {
                    if e.kind() != io::ErrorKind::NotFound {
                        error!("Error serving {:?}: {}", path, e);
                    }
                    continue;
                }
            }
        }
        return None;
    }
}

#[cfg(test)]
mod test {
    use std::mem::size_of;
    use accept_encoding::{AcceptEncodingParser};
    use super::*;

    fn send<T: Send>(_: &T) {}
    fn self_contained<T: 'static>(_: &T) {}

    #[test]
    fn traits() {
        let v = Input {
            mode: Mode::Get,
            accept_encoding: AcceptEncodingParser::new().done(),
            range: None,
            if_range: None,
            if_match: Vec::new(),
            if_none: Vec::new(),
            if_unmodified: None,
            if_modified: None,
        };
        send(&v);
        self_contained(&v);
    }

    #[cfg(target_arch="x86_64")]
    #[test]
    fn size() {
        assert_eq!(size_of::<Range>(), 24);
        assert_eq!(size_of::<Input>(), 168);
    }
}
