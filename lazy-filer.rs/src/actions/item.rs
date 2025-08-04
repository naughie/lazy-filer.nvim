use crate::fs::Permissions;

use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub struct Level(usize);

impl Level {
    pub fn increment(self) -> Self {
        Self(self.0 + 1)
    }

    pub fn base() -> Self {
        Self(0)
    }

    pub fn to_num(self) -> usize {
        self.0
    }

    pub fn repeat(self, mut f: impl FnMut()) {
        let level = self.0;
        for _ in 0..level {
            f();
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FileType {
    Regular,
    Directory,
    Other,
}
impl FileType {
    fn to_s(self) -> u8 {
        match self {
            Self::Regular => b'F',
            Self::Directory => b'D',
            Self::Other => b'U',
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Metadata {
    pub perm: Permissions,
    pub file_type: FileType,
}
impl Metadata {
    pub fn push(self, s: &mut String) {
        s.push('[');
        let perm = self.perm.to_s();
        let ftype = self.file_type.to_s();
        let meta = [perm[0], perm[1], perm[2], ftype];
        let meta = unsafe { std::str::from_utf8_unchecked(&meta) };
        s.push_str(meta);
        s.push(']');
        s.push(' ');
    }

    pub fn is_regular(self) -> bool {
        matches!(self.file_type, FileType::Regular)
    }

    pub fn is_dir(self) -> bool {
        matches!(self.file_type, FileType::Directory)
    }
}

#[derive(Debug, Clone)]
pub struct Item {
    pub level: Level,
    pub path: PathBuf,
    pub metadata: Metadata,
}
