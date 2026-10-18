// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A git client that does one thing: fetch a commit and hand its files
//! over. It speaks version 2 of the protocol over HTTP(S), reads the pack
//! the server answers with, and walks the commit's tree into a [`Tree`].

use std::collections::HashMap;
use std::io::Read;
use std::rc::Rc;

use db::dependencies::FetchError;
use sha2::Digest;

use crate::archive::MAX_UNPACKED;
use crate::tree::{Entry, Tree};

/// Which hash a repository names its objects with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Sha1,
    Sha256,
}

impl Format {
    pub fn byte_length(self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
        }
    }

    pub fn formatted_length(self) -> usize {
        2 * self.byte_length()
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "sha1" => Some(Self::Sha1),
            "sha256" => Some(Self::Sha256),
            _ => None,
        }
    }

    /// The ID of an object: the hash of its kind, its length and its data.
    fn hash_object(self, kind: ObjectType, data: &[u8]) -> Oid {
        let header = format!("{} {}\0", kind.name(), data.len());
        let bytes = match self {
            Self::Sha1 => {
                let mut hasher = sha1::Sha1::new();
                hasher.update(header.as_bytes());
                hasher.update(data);
                hasher.finalize().to_vec()
            }
            Self::Sha256 => {
                let mut hasher = sha2::Sha256::new();
                hasher.update(header.as_bytes());
                hasher.update(data);
                hasher.finalize().to_vec()
            }
        };
        Oid { bytes }
    }
}

/// The ID of a git object.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Oid {
    bytes: Vec<u8>,
}

impl Oid {
    pub fn format(&self) -> Format {
        match self.bytes.len() {
            20 => Format::Sha1,
            _ => Format::Sha256,
        }
    }

    fn from_bytes(format: Format, bytes: &[u8]) -> Self {
        debug_assert_eq!(bytes.len(), format.byte_length());
        Self {
            bytes: bytes.to_vec(),
        }
    }

    pub fn parse(format: Format, s: &str) -> Result<Self, Error> {
        if s.len() != format.formatted_length() || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::InvalidOid);
        }
        let bytes = (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|_| Error::InvalidOid)?;
        Ok(Self { bytes })
    }

    /// An ID of either format, told apart by its length.
    pub fn parse_any(s: &str) -> Result<Self, Error> {
        if s.len() == Format::Sha1.formatted_length() {
            Self::parse(Format::Sha1, s)
        } else if s.len() == Format::Sha256.formatted_length() {
            Self::parse(Format::Sha256, s)
        } else {
            Err(Error::InvalidOid)
        }
    }
}

impl std::fmt::Display for Oid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.bytes {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// What can go wrong in the protocol or in a pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidOid,
    InvalidPacket,
    UnexpectedPacket,
    UnsupportedProtocol,
    ProtocolError,
    UnparseableRedirect,
    InvalidRefPacket,
    InvalidHeader,
    UnsupportedVersion,
    InvalidFormat,
    InvalidObject,
    ObjectTooLarge,
    IncompletePack,
    InvalidDeltaInstruction,
    ObjectNotFound,
    NotACommit,
    NotATree,
    InvalidCommit,
    InvalidTree,
    InvalidFile,
    EndOfStream,
    /// The transport's own words.
    Http(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(error) => f.write_str(error),
            other => write!(f, "{other:?}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObjectType {
    Commit,
    Tree,
    Blob,
    Tag,
}

impl ObjectType {
    fn name(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Tree => "tree",
            Self::Blob => "blob",
            Self::Tag => "tag",
        }
    }
}

#[derive(Clone)]
struct Object {
    kind: ObjectType,
    data: Rc<[u8]>,
}

/// Reads through a slice, a byte or an integer at a time.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn at(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).ok_or(Error::EndOfStream)?;
        let bytes = self.data.get(self.pos..end).ok_or(Error::EndOfStream)?;
        self.pos = end;
        Ok(bytes)
    }

    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    fn u32_be(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    /// An unsigned LEB128 integer.
    fn leb128(&mut self) -> Result<u64, Error> {
        let mut value = 0u64;
        let mut shift = 0u32;
        loop {
            let byte = self.byte()?;
            let part = u64::from(byte & 0x7f);
            if shift >= 64 || (part << shift) >> shift != part {
                return Err(Error::InvalidFormat);
            }
            value |= part << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
    }
}

/// The header of an entry of a pack, as
/// [pack-format](https://git-scm.com/docs/pack-format) documents it.
enum EntryHeader {
    Undeltified { kind: ObjectType, size: u64 },
    OfsDelta { offset: u64, size: u64 },
    RefDelta { base: Oid, size: u64 },
}

impl EntryHeader {
    fn read(format: Format, cursor: &mut Cursor<'_>) -> Result<Self, Error> {
        let initial = cursor.byte().map_err(|_| Error::InvalidFormat)?;
        let rest = match initial & 0x80 {
            0 => 0,
            _ => cursor.leb128()?,
        };
        if rest > u64::MAX >> 4 {
            return Err(Error::InvalidFormat);
        }
        let size = u64::from(initial & 0x0f) | rest << 4;
        let undeltified = |kind| Self::Undeltified { kind, size };
        Ok(match (initial >> 4) & 0x07 {
            1 => undeltified(ObjectType::Commit),
            2 => undeltified(ObjectType::Tree),
            3 => undeltified(ObjectType::Blob),
            4 => undeltified(ObjectType::Tag),
            6 => Self::OfsDelta {
                offset: read_offset_var_int(cursor)?,
                size,
            },
            7 => Self::RefDelta {
                base: Oid::from_bytes(
                    format,
                    cursor
                        .take(format.byte_length())
                        .map_err(|_| Error::InvalidFormat)?,
                ),
                size,
            },
            _ => return Err(Error::InvalidFormat),
        })
    }

    fn size(&self) -> u64 {
        match self {
            Self::Undeltified { size, .. }
            | Self::OfsDelta { size, .. }
            | Self::RefDelta { size, .. } => *size,
        }
    }
}

/// The distance from a delta to its base, in the encoding of its own that
/// the pack format gives it.
fn read_offset_var_int(cursor: &mut Cursor<'_>) -> Result<u64, Error> {
    let mut byte = cursor.byte()?;
    let mut value = u64::from(byte & 0x7f);
    while byte & 0x80 != 0 {
        byte = cursor.byte()?;
        value = value
            .checked_add(1)
            .filter(|value| *value <= u64::MAX >> 7)
            .ok_or(Error::InvalidFormat)?
            << 7;
        value |= u64::from(byte & 0x7f);
    }
    Ok(value)
}

/// The `size` bytes a zlib stream at `pos` of `pack` inflates to, and how
/// many bytes of the pack the stream takes.
fn inflate(pack: &[u8], pos: usize, size: u64) -> Result<(Vec<u8>, usize), Error> {
    if size > MAX_UNPACKED {
        return Err(Error::ObjectTooLarge);
    }
    let input = pack.get(pos..).ok_or(Error::InvalidObject)?;
    let mut decoder = flate2::read::ZlibDecoder::new(input);
    let mut data = Vec::with_capacity(size as usize);
    // One byte more than announced, to tell a longer object from this one.
    (&mut decoder)
        .take(size + 1)
        .read_to_end(&mut data)
        .map_err(|_| Error::InvalidObject)?;
    if data.len() as u64 != size {
        return Err(Error::InvalidObject);
    }
    Ok((data, decoder.total_in() as usize))
}

/// The objects of a pack, each found by its ID.
pub struct Repository {
    format: Format,
    pack: Vec<u8>,
    /// Where each object starts in the pack.
    index: HashMap<Oid, usize>,
    /// The objects read so far, by where they start: a delta is resolved
    /// once, and those built on it start from there.
    cache: HashMap<usize, Object>,
}

impl Repository {
    /// Indexes `pack`: every object gets its ID, the ones written whole
    /// from their data and the deltas from what they resolve to.
    pub fn new(format: Format, pack: Vec<u8>) -> Result<Self, Error> {
        let mut repository = Self {
            format,
            pack,
            index: HashMap::new(),
            cache: HashMap::new(),
        };
        let mut pending_deltas = repository.index_first_pass()?;
        let mut remaining = pending_deltas.len();
        while remaining > 0 {
            let mut i = remaining;
            while i > 0 {
                i -= 1;
                if let Some(oid) = repository.hash_delta(pending_deltas[i])? {
                    repository.index.insert(oid, pending_deltas[i]);
                    pending_deltas.swap_remove(i);
                }
            }
            if pending_deltas.len() == remaining {
                return Err(Error::IncompletePack);
            }
            remaining = pending_deltas.len();
        }
        Ok(repository)
    }

    /// Indexes the objects written whole, and answers where the deltas
    /// are.
    fn index_first_pass(&mut self) -> Result<Vec<usize>, Error> {
        let mut cursor = Cursor::at(&self.pack, 0);
        if cursor.take(4).map_err(|_| Error::InvalidHeader)? != b"PACK" {
            return Err(Error::InvalidHeader);
        }
        if cursor.u32_be().map_err(|_| Error::InvalidHeader)? != 2 {
            return Err(Error::UnsupportedVersion);
        }
        let total_objects = cursor.u32_be().map_err(|_| Error::InvalidHeader)?;

        let mut pending_deltas = Vec::new();
        let mut index = Vec::new();
        for _ in 0..total_objects {
            let entry_offset = cursor.pos;
            let header = EntryHeader::read(self.format, &mut cursor)?;
            let (data, consumed) = inflate(&self.pack, cursor.pos, header.size())?;
            cursor.pos += consumed;
            match header {
                EntryHeader::Undeltified { kind, .. } => {
                    index.push((self.format.hash_object(kind, &data), entry_offset));
                }
                EntryHeader::OfsDelta { .. } | EntryHeader::RefDelta { .. } => {
                    pending_deltas.push(entry_offset);
                }
            }
        }
        self.index.extend(index);
        Ok(pending_deltas)
    }

    /// The ID of the delta at `offset`, or `None` while the object it is
    /// built on, named by its ID, is not indexed yet.
    fn hash_delta(&mut self, offset: usize) -> Result<Option<Oid>, Error> {
        match self.read_object_at(offset, false)? {
            Some(object) => Ok(Some(self.format.hash_object(object.kind, &object.data))),
            None => Ok(None),
        }
    }

    /// The object at `offset`, its deltas applied. With `indexed`, every
    /// object a delta names is known: one that is not is an error and not
    /// a `None`.
    fn read_object_at(&mut self, offset: usize, indexed: bool) -> Result<Option<Object>, Error> {
        // The chain of deltas down to an object written whole.
        let mut base_offset = offset;
        let mut delta_offsets = Vec::new();
        let base_object = loop {
            if let Some(object) = self.cache.get(&base_offset) {
                break object.clone();
            }
            let mut cursor = Cursor::at(&self.pack, base_offset);
            let header = EntryHeader::read(self.format, &mut cursor)?;
            match header {
                EntryHeader::OfsDelta { offset, .. } => {
                    delta_offsets.push(base_offset);
                    base_offset = usize::try_from(offset)
                        .ok()
                        .and_then(|offset| base_offset.checked_sub(offset))
                        .ok_or(Error::InvalidFormat)?;
                }
                EntryHeader::RefDelta { base, .. } => {
                    delta_offsets.push(base_offset);
                    base_offset = match self.index.get(&base) {
                        Some(offset) => *offset,
                        None if indexed => return Err(Error::ObjectNotFound),
                        None => return Ok(None),
                    };
                }
                EntryHeader::Undeltified { kind, size } => {
                    let (data, _) = inflate(&self.pack, cursor.pos, size)?;
                    let object = Object {
                        kind,
                        data: data.into(),
                    };
                    self.cache.insert(base_offset, object.clone());
                    break object;
                }
            }
            // A chain cannot be longer than the pack has objects.
            if delta_offsets.len() > self.pack.len() {
                return Err(Error::InvalidFormat);
            }
        };

        // The deltas, from the one next to the base to the object asked
        // for.
        let mut object = base_object;
        for delta_offset in delta_offsets.into_iter().rev() {
            let mut cursor = Cursor::at(&self.pack, delta_offset);
            let header = EntryHeader::read(self.format, &mut cursor)?;
            let (delta, _) = inflate(&self.pack, cursor.pos, header.size())?;
            let mut delta = Cursor::at(&delta, 0);
            delta.leb128()?; // the length of the base
            let expanded_size = delta.leb128()?;
            if expanded_size > MAX_UNPACKED {
                return Err(Error::ObjectTooLarge);
            }
            let expanded = expand_delta(&object.data, &mut delta, expanded_size as usize)?;
            object = Object {
                kind: object.kind,
                data: expanded.into(),
            };
            self.cache.insert(delta_offset, object.clone());
        }
        Ok(Some(object))
    }

    fn read_object(&mut self, oid: &Oid) -> Result<Object, Error> {
        let offset = *self.index.get(oid).ok_or(Error::ObjectNotFound)?;
        self.read_object_at(offset, true)?
            .ok_or(Error::ObjectNotFound)
    }

    /// The files of the commit `commit_oid`.
    pub fn checkout(&mut self, commit_oid: &Oid) -> Result<Tree, Error> {
        let commit = self.read_object(commit_oid)?;
        if commit.kind != ObjectType::Commit {
            return Err(Error::NotACommit);
        }
        let tree_oid = self.commit_tree(&commit.data)?;
        let mut tree = Tree {
            root_error_message: "unable to unpack packfile",
            ..Tree::default()
        };
        self.checkout_tree(&tree_oid, "", &mut tree, 0)?;
        Ok(tree)
    }

    /// The ID of the tree a commit is of: its first line.
    fn commit_tree(&self, commit_data: &[u8]) -> Result<Oid, Error> {
        let hex = self.format.formatted_length();
        let rest = commit_data
            .strip_prefix(b"tree ")
            .ok_or(Error::InvalidCommit)?;
        if rest.len() <= hex || rest[hex] != b'\n' {
            return Err(Error::InvalidCommit);
        }
        let hex = std::str::from_utf8(&rest[..hex]).map_err(|_| Error::InvalidCommit)?;
        Oid::parse(self.format, hex).map_err(|_| Error::InvalidCommit)
    }

    fn checkout_tree(
        &mut self,
        tree_oid: &Oid,
        current_path: &str,
        tree: &mut Tree,
        depth: usize,
    ) -> Result<(), Error> {
        // A tree that holds itself would never end.
        if depth > 128 {
            return Err(Error::InvalidTree);
        }
        let object = self.read_object(tree_oid)?;
        if object.kind != ObjectType::Tree {
            return Err(Error::NotATree);
        }
        let mut entries = TreeIterator {
            format: self.format,
            data: &object.data,
            pos: 0,
        };
        while let Some(entry) = entries.next()? {
            let path = match current_path {
                "" => entry.name.to_string(),
                dir => format!("{dir}/{}", entry.name),
            };
            match entry.kind {
                TreeEntryKind::Directory => {
                    self.checkout_tree(&entry.oid, &path, tree, depth + 1)?
                }
                TreeEntryKind::File => {
                    let file = self.read_object(&entry.oid)?;
                    if file.kind != ObjectType::Blob {
                        return Err(Error::InvalidFile);
                    }
                    tree.insert(path, Entry::File(file.data.to_vec()));
                }
                TreeEntryKind::Symlink => {
                    let link = self.read_object(&entry.oid)?;
                    if link.kind != ObjectType::Blob {
                        return Err(Error::InvalidFile);
                    }
                    let link =
                        String::from_utf8(link.data.to_vec()).map_err(|_| Error::InvalidFile)?;
                    tree.insert(path, Entry::Link(link));
                }
                // Another repository's commit: `git archive` leaves an
                // empty folder there, and an empty folder is in no package.
                TreeEntryKind::Gitlink => {}
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TreeEntryKind {
    Directory,
    File,
    Symlink,
    Gitlink,
}

struct TreeEntry<'a> {
    kind: TreeEntryKind,
    name: &'a str,
    oid: Oid,
}

/// The entries of a tree object: a mode in octal, a space, a name, a zero
/// byte and an ID, one after the other.
struct TreeIterator<'a> {
    format: Format,
    data: &'a [u8],
    pos: usize,
}

impl<'a> TreeIterator<'a> {
    fn next(&mut self) -> Result<Option<TreeEntry<'a>>, Error> {
        if self.pos == self.data.len() {
            return Ok(None);
        }
        let rest = &self.data[self.pos..];
        let mode_end = rest
            .iter()
            .position(|b| *b == b' ')
            .ok_or(Error::InvalidTree)?;
        let mode = std::str::from_utf8(&rest[..mode_end])
            .ok()
            .and_then(|mode| u16::from_str_radix(mode, 8).ok())
            .ok_or(Error::InvalidTree)?;
        let kind = match mode >> 12 {
            0o04 => TreeEntryKind::Directory,
            0o10 => TreeEntryKind::File,
            0o12 => TreeEntryKind::Symlink,
            0o16 => TreeEntryKind::Gitlink,
            _ => return Err(Error::InvalidTree),
        };
        let is_file = kind == TreeEntryKind::File;
        match mode & 0o777 {
            0 if !is_file => {}
            0o644 | 0o755 if is_file => {}
            _ => return Err(Error::InvalidTree),
        }

        let rest = &rest[mode_end + 1..];
        let name_end = rest
            .iter()
            .position(|b| *b == 0)
            .ok_or(Error::InvalidTree)?;
        let name = std::str::from_utf8(&rest[..name_end]).map_err(|_| Error::InvalidTree)?;
        // One part of a path, and one that is the same on every system: a
        // server does not get to write outside the package.
        if matches!(name, "" | "." | "..")
            || name.contains(['/', '\\', ':'])
            || name.contains(char::is_control)
        {
            return Err(Error::InvalidTree);
        }

        let oid_length = self.format.byte_length();
        let oid = rest
            .get(name_end + 1..name_end + 1 + oid_length)
            .ok_or(Error::InvalidTree)?;
        self.pos += mode_end + 1 + name_end + 1 + oid_length;
        Ok(Some(TreeEntry {
            kind,
            name,
            oid: Oid::from_bytes(self.format, oid),
        }))
    }
}

/// Applies a delta to `base`: its instructions copy a run of the base or
/// insert bytes of their own, up to `expanded_size` bytes.
fn expand_delta(
    base: &[u8],
    delta: &mut Cursor<'_>,
    expanded_size: usize,
) -> Result<Vec<u8>, Error> {
    let mut out = Vec::with_capacity(expanded_size);
    while delta.pos < delta.data.len() {
        let inst = delta.byte()?;
        if inst & 0x80 != 0 {
            let mut offset = 0usize;
            for i in 0..4 {
                if inst & (1 << i) != 0 {
                    offset |= usize::from(delta.byte()?) << (8 * i);
                }
            }
            let mut size = 0usize;
            for i in 0..3 {
                if inst & (1 << (4 + i)) != 0 {
                    size |= usize::from(delta.byte()?) << (8 * i);
                }
            }
            if size == 0 {
                size = 0x10000;
            }
            let run = offset
                .checked_add(size)
                .and_then(|end| base.get(offset..end))
                .ok_or(Error::InvalidObject)?;
            out.extend_from_slice(run);
        } else if inst != 0 {
            out.extend_from_slice(delta.take(usize::from(inst))?);
        } else {
            return Err(Error::InvalidDeltaInstruction);
        }
        if out.len() > expanded_size {
            return Err(Error::InvalidObject);
        }
    }
    if out.len() != expanded_size {
        return Err(Error::InvalidObject);
    }
    Ok(out)
}

/// One packet of the protocol, in the pkt-line format of
/// [protocol-common](https://git-scm.com/docs/protocol-common).
#[derive(Debug, PartialEq, Eq)]
enum Packet<'a> {
    Flush,
    Delimiter,
    ResponseEnd,
    Data(&'a [u8]),
}

impl<'a> Packet<'a> {
    const MAX_DATA_LENGTH: usize = 65516;

    fn read(cursor: &mut Cursor<'a>) -> Result<Self, Error> {
        let length = std::str::from_utf8(cursor.take(4)?)
            .ok()
            .and_then(|length| u16::from_str_radix(length, 16).ok())
            .ok_or(Error::InvalidPacket)?;
        match length {
            0 => Ok(Self::Flush),
            1 => Ok(Self::Delimiter),
            2 => Ok(Self::ResponseEnd),
            3 => Err(Error::InvalidPacket),
            length if usize::from(length) - 4 > Self::MAX_DATA_LENGTH => Err(Error::InvalidPacket),
            length => Ok(Self::Data(cursor.take(usize::from(length) - 4)?)),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Flush => out.extend_from_slice(b"0000"),
            Self::Delimiter => out.extend_from_slice(b"0001"),
            Self::ResponseEnd => out.extend_from_slice(b"0002"),
            Self::Data(data) => {
                assert!(data.len() <= Self::MAX_DATA_LENGTH);
                out.extend_from_slice(format!("{:04x}", data.len() + 4).as_bytes());
                out.extend_from_slice(data);
            }
        }
    }

    /// A line of text without the `\n` it may end with.
    fn normalize_text(data: &[u8]) -> &[u8] {
        data.strip_suffix(b"\n").unwrap_or(data)
    }
}

fn text(line: &str) -> Packet<'_> {
    Packet::Data(line.as_bytes())
}

/// A ref of the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ref {
    pub oid: Oid,
    pub name: String,
    pub symref_target: Option<String>,
    /// The commit an annotated tag points at.
    pub peeled: Option<Oid>,
}

/// A session with a git server, over HTTP(S) and in version 2 of the
/// protocol, as [protocol-v2](https://git-scm.com/docs/protocol-v2)
/// documents it.
pub struct Session {
    agent: ureq::Agent,
    /// The repository's URL, with no `/` at its end.
    location: String,
    supports_agent: bool,
    supports_shallow: bool,
    pub object_format: Format,
}

const AGENT_CAPABILITY: &str = concat!("agent=rk/", env!("CARGO_PKG_VERSION"), "\n");

impl Session {
    /// Asks the server at `url` what it can do.
    pub fn init(url: &str) -> Result<Self, Error> {
        let mut session = Self {
            agent: crate::agent(3),
            location: url.trim_end_matches('/').to_string(),
            supports_agent: false,
            supports_shallow: false,
            object_format: Format::Sha1,
        };
        let response = session.capabilities()?;
        let mut cursor = Cursor::at(&response, 0);
        // Some servers say `# service=git-upload-pack` first. What is
        // looked for is the answer that starts with `version 2`.
        let mut at_start = true;
        loop {
            match Packet::read(&mut cursor) {
                Err(Error::EndOfStream) => return Err(Error::UnsupportedProtocol),
                Err(error) => return Err(error),
                Ok(Packet::Flush) => at_start = true,
                Ok(Packet::Data(data)) => {
                    if at_start && Packet::normalize_text(data) == b"version 2" {
                        break;
                    }
                    at_start = false;
                }
                Ok(_) => return Err(Error::UnexpectedPacket),
            }
        }
        loop {
            let capability = match Packet::read(&mut cursor)? {
                Packet::Flush => break,
                Packet::Data(data) => String::from_utf8_lossy(Packet::normalize_text(data)),
                _ => return Err(Error::UnexpectedPacket),
            };
            let (key, value) = match capability.split_once('=') {
                Some((key, value)) => (key, Some(value)),
                None => (&*capability, None),
            };
            match (key, value) {
                ("agent", _) => session.supports_agent = true,
                ("fetch", Some(features)) => {
                    session.supports_shallow = features.split(' ').any(|f| f == "shallow");
                }
                ("object-format", Some(format)) => {
                    if let Some(format) = Format::from_name(format) {
                        session.object_format = format;
                    }
                }
                _ => {}
            }
        }
        Ok(session)
    }

    /// The server's answer to the first request. A server that sends the
    /// request elsewhere is followed there, for this request and the next.
    fn capabilities(&mut self) -> Result<Vec<u8>, Error> {
        use ureq::ResponseExt;
        let url = format!("{}/info/refs?service=git-upload-pack", self.location);
        let response = self
            .agent
            .get(&url)
            .header("Git-Protocol", "version=2")
            .call()
            .map_err(|error| Error::Http(error.to_string()))?;
        let arrived = response.get_uri().to_string();
        if arrived != url {
            let arrived = arrived.split(['?', '#']).next().unwrap_or_default();
            self.location = arrived
                .strip_suffix("/info/refs")
                .ok_or(Error::UnparseableRedirect)?
                .to_string();
        }
        crate::body(response).map_err(Error::Http)
    }

    /// One command sent to the server, and its answer.
    fn command(&self, body: &[u8]) -> Result<Vec<u8>, Error> {
        let url = format!("{}/git-upload-pack", self.location);
        let response = self
            .agent
            .post(&url)
            .config()
            .max_redirects(0)
            .build()
            .header("Content-Type", "application/x-git-upload-pack-request")
            .header("Git-Protocol", "version=2")
            .send(body)
            .map_err(|error| Error::Http(error.to_string()))?;
        crate::body(response).map_err(Error::Http)
    }

    fn start(&self, command: &str) -> Vec<u8> {
        let mut body = Vec::new();
        text(command).write(&mut body);
        if self.supports_agent {
            text(AGENT_CAPABILITY).write(&mut body);
        }
        text(&format!("object-format={}\n", self.object_format.name())).write(&mut body);
        Packet::Delimiter.write(&mut body);
        body
    }

    /// The refs of the server whose name starts with one of `ref_prefixes`.
    /// A server may answer with others: the caller keeps the ones it asked
    /// for.
    pub fn list_refs(
        &self,
        ref_prefixes: &[&str],
        include_peeled: bool,
    ) -> Result<Vec<Ref>, Error> {
        let mut body = self.start("command=ls-refs\n");
        for prefix in ref_prefixes {
            text(&format!("ref-prefix {prefix}\n")).write(&mut body);
        }
        if include_peeled {
            text("peel\n").write(&mut body);
        }
        Packet::Flush.write(&mut body);

        let response = self.command(&body)?;
        let mut cursor = Cursor::at(&response, 0);
        let mut refs = Vec::new();
        loop {
            let data = match Packet::read(&mut cursor)? {
                Packet::Flush => return Ok(refs),
                Packet::Data(data) => Packet::normalize_text(data),
                _ => return Err(Error::UnexpectedPacket),
            };
            let line = std::str::from_utf8(data).map_err(|_| Error::InvalidRefPacket)?;
            let mut parts = line.split(' ');
            let oid = parts.next().ok_or(Error::InvalidRefPacket)?;
            let oid = Oid::parse(self.object_format, oid).map_err(|_| Error::InvalidRefPacket)?;
            let name = parts.next().ok_or(Error::InvalidRefPacket)?.to_string();
            let mut symref_target = None;
            let mut peeled = None;
            for attribute in parts {
                if let Some(target) = attribute.strip_prefix("symref-target:") {
                    symref_target = Some(target.to_string());
                } else if let Some(oid) = attribute.strip_prefix("peeled:") {
                    peeled = Some(
                        Oid::parse(self.object_format, oid).map_err(|_| Error::InvalidRefPacket)?,
                    );
                }
            }
            refs.push(Ref {
                oid,
                name,
                symref_target,
                peeled,
            });
        }
    }

    /// The pack that holds `wants`, without their history when the server
    /// can leave it out.
    pub fn fetch(&self, wants: &[&str]) -> Result<Vec<u8>, Error> {
        let mut body = self.start("command=fetch\n");
        // The pack reader takes deltas by offset, and shows no progress.
        text("ofs-delta\n").write(&mut body);
        text("no-progress\n").write(&mut body);
        if self.supports_shallow {
            text("deepen 1\n").write(&mut body);
        }
        for want in wants {
            text(&format!("want {want}\n")).write(&mut body);
        }
        text("done\n").write(&mut body);
        Packet::Flush.write(&mut body);

        let response = self.command(&body)?;
        let mut cursor = Cursor::at(&response, 0);
        // The answer comes in sections, and only the pack is of use: a
        // fresh fetch has nothing to negotiate.
        let mut at_section_start = true;
        loop {
            match (at_section_start, Packet::read(&mut cursor)?) {
                (true, Packet::Data(data)) if Packet::normalize_text(data) == b"packfile" => break,
                (true, Packet::Data(_)) => at_section_start = false,
                (true, _) => return Err(Error::UnexpectedPacket),
                (false, Packet::Delimiter) => at_section_start = true,
                (false, Packet::Data(_)) => {}
                (false, _) => return Err(Error::UnexpectedPacket),
            }
        }
        // Each packet of the pack says what it carries with its first
        // byte: the pack, a line of progress, or an error.
        let mut pack = Vec::new();
        loop {
            match Packet::read(&mut cursor)? {
                Packet::Flush => return Ok(pack),
                Packet::Data(data) if data.len() > 1 => match data[0] {
                    1 => pack.extend_from_slice(&data[1..]),
                    3 => return Err(Error::ProtocolError),
                    _ => {}
                },
                Packet::Data(_) => {}
                _ => return Err(Error::UnexpectedPacket),
            }
        }
    }
}

/// The commit `want_ref` names on the server: an ID as it is, and
/// otherwise the ref of that name, the branch or the tag.
pub fn resolve(session: &Session, want_ref: &str) -> Result<Oid, FetchError> {
    if let Ok(oid) = Oid::parse_any(want_ref) {
        return Ok(oid);
    }
    let head = format!("refs/heads/{want_ref}");
    let tag = format!("refs/tags/{want_ref}");
    let refs = session
        .list_refs(&[want_ref, &head, &tag], true)
        .map_err(|error| FetchError::new(format!("unable to list refs: {error}")))?;
    refs.into_iter()
        .find(|r| r.name == want_ref || r.name == head || r.name == tag)
        .map(|r| r.peeled.unwrap_or(r.oid))
        .ok_or_else(|| FetchError::new(format!("ref not found: {want_ref}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packet_is_read_as_it_is_written() {
        let mut out = Vec::new();
        text("command=ls-refs\n").write(&mut out);
        Packet::Delimiter.write(&mut out);
        Packet::Flush.write(&mut out);
        assert_eq!(out, b"0014command=ls-refs\n00010000");
        let mut cursor = Cursor::at(&out, 0);
        assert_eq!(
            Packet::read(&mut cursor),
            Ok(Packet::Data(b"command=ls-refs\n"))
        );
        assert_eq!(Packet::read(&mut cursor), Ok(Packet::Delimiter));
        assert_eq!(Packet::read(&mut cursor), Ok(Packet::Flush));
        assert_eq!(Packet::read(&mut cursor), Err(Error::EndOfStream));
        assert_eq!(
            Packet::read(&mut Cursor::at(b"0003", 0)),
            Err(Error::InvalidPacket)
        );
        assert_eq!(
            Packet::read(&mut Cursor::at(b"zzzz", 0)),
            Err(Error::InvalidPacket)
        );
    }
}
