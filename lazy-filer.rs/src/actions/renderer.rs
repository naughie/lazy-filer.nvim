use super::{NvimErr, NvimWtr};
use nvim_router::nvim_rs::{Buffer, Neovim};

use crate::fs::Permissions;

use std::ops::Add;
use std::ops::RangeBounds;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::MutexGuard;

use futures::stream::{Stream, StreamExt as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

    pub const MAX: Self = Self(10);
}

#[derive(Debug, Clone, Copy)]
pub enum FileType {
    Regular,
    Directory,
    LinkRegular,
    LinkDirectory,
    LinkOther,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub struct Metadata {
    pub perm: Permissions,
    pub file_type: FileType,
}
impl Metadata {
    pub fn is_regular(self) -> bool {
        matches!(self.file_type, FileType::Regular | FileType::LinkRegular)
    }

    pub fn is_dir(self) -> bool {
        matches!(
            self.file_type,
            FileType::Directory | FileType::LinkDirectory
        )
    }

    pub fn is_link(self) -> bool {
        matches!(
            self.file_type,
            FileType::LinkRegular | FileType::LinkDirectory | FileType::LinkOther
        )
    }

    fn is_executable(self) -> bool {
        self.is_regular() && self.perm.exec
    }
}

#[derive(Debug, Clone)]
pub struct Item {
    pub level: Level,
    pub path: PathBuf,
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Default)]
pub struct Items(Arc<Mutex<Vec<Item>>>);

impl Items {
    async fn lock(&self) -> MutexGuard<'_, Vec<Item>> {
        self.0.lock().await
    }

    pub fn edit<'n, 'b>(
        &self,
        nvim: &'n Neovim<NvimWtr>,
        buf: &'b Buffer<NvimWtr>,
    ) -> Edit<'_, 'n, 'b> {
        Edit {
            inner: self,
            nvim,
            buf,
        }
    }

    pub fn get(&self, idx: LineIdx) -> PathGetter<'_> {
        PathGetter { inner: self, idx }
    }

    pub fn iter(&self) -> ItemIter<'_> {
        ItemIter { inner: self }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LineIdx(i64);

impl LineIdx {
    fn as_usize(self, len: usize) -> Option<usize> {
        let idx = self.0;
        if idx >= 0 {
            Some(idx as usize)
        } else {
            let idx = (len as i64) + idx;
            if idx >= 0 { Some(idx as usize) } else { None }
        }
    }
}

impl From<i64> for LineIdx {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl Add<i64> for LineIdx {
    type Output = Self;

    fn add(self, rhs: i64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

pub struct Edit<'a, 'n, 'b> {
    inner: &'a Items,
    nvim: &'n Neovim<NvimWtr>,
    buf: &'b Buffer<NvimWtr>,
}

impl Edit<'_, '_, '_> {
    pub async fn replace_all(self, lines: impl Stream<Item = Item>) -> Result<(), NvimErr> {
        let lines = lines.collect::<Vec<_>>().await;

        let rendered = lines.iter().map(make_line).collect();

        let hl_ranges = make_hl_ranges(0, &lines);

        let mut lock = self.inner.lock().await;
        *lock = lines;
        drop(lock);

        self.buf.set_lines(0, -1, false, rendered).await?;
        highlight(self.nvim, hl_ranges).await?;

        Ok(())
    }

    pub async fn replace_range<St, Func, Range>(self, lines: St, range: Func) -> Result<(), NvimErr>
    where
        St: Stream<Item = Item>,
        Func: for<'a> FnOnce(&'a [Item]) -> Range,
        Range: RangeBounds<usize>,
    {
        use std::ops::Bound;

        let lines = lines.collect::<Vec<_>>().await;
        let rendered = lines.iter().map(make_line).collect();

        let mut lock = self.inner.lock().await;
        let range = range(&lock);

        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => lock.len(),
        };

        let hl_ranges = make_hl_ranges(start as i64, &lines);

        lock.splice(start..end, lines);
        drop(lock);

        self.buf
            .set_lines(start as i64, end as i64, false, rendered)
            .await?;
        highlight(self.nvim, hl_ranges).await?;

        Ok(())
    }

    pub async fn insert(self, lines: impl Stream<Item = Item>, at: LineIdx) -> Result<(), NvimErr> {
        let lines = lines.collect::<Vec<_>>().await;
        let rendered = lines.iter().map(make_line).collect();

        let hl_ranges = {
            let LineIdx(at) = at;
            make_hl_ranges(at, &lines)
        };

        let mut lock = self.inner.lock().await;
        if let Some(at) = at.as_usize(lock.len()) {
            lock.splice(at..at, lines);
        }
        drop(lock);

        let LineIdx(at) = at;
        self.buf.set_lines(at, at, false, rendered).await?;
        highlight(self.nvim, hl_ranges).await?;

        Ok(())
    }

    pub async fn insert_dyn(
        self,
        item: Item,
        at: impl for<'a> FnOnce(&'a [Item]) -> usize,
    ) -> Result<(), NvimErr> {
        let mut lock = self.inner.lock().await;

        let at = at(&lock);

        let hl_ranges = make_hl_ranges(at as i64, [&item]);

        self.buf
            .set_lines(at as i64, at as i64, false, vec![make_line(&item)])
            .await?;

        highlight(self.nvim, hl_ranges).await?;
        lock.insert(at, item);

        Ok(())
    }

    pub async fn remove(self, at: LineIdx) -> Result<(), NvimErr> {
        let mut lock = self.inner.lock().await;

        if let Some(at) = at.as_usize(lock.len()) {
            lock.remove(at);
        }
        drop(lock);

        let LineIdx(at) = at;
        self.buf.set_lines(at, at + 1, false, vec![]).await?;

        Ok(())
    }

    pub async fn remove_range<Func, Range>(self, range: Func) -> Result<(), NvimErr>
    where
        Func: for<'a> FnOnce(&'a [Item]) -> Range,
        Range: RangeBounds<usize>,
    {
        use std::ops::Bound;

        let mut lock = self.inner.lock().await;

        let range = range(&lock);

        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => lock.len(),
        };

        lock.drain(start..end);
        drop(lock);

        self.buf
            .set_lines(start as i64, end as i64, false, vec![])
            .await?;

        Ok(())
    }
}

pub struct PathGetter<'a> {
    inner: &'a Items,
    idx: LineIdx,
}

impl PathGetter<'_> {
    pub async fn and_then<Func, T>(self, f: Func) -> Option<T>
    where
        Func: for<'p> FnOnce(&'p Item) -> Option<T>,
    {
        let lock = self.inner.lock().await;
        self.idx
            .as_usize(lock.len())
            .and_then(|idx| lock.get(idx))
            .and_then(f)
    }
}

pub struct ItemIter<'a> {
    inner: &'a Items,
}

impl ItemIter<'_> {
    pub async fn fold<B, F>(self, init: B, f: F) -> B
    where
        F: for<'p> FnMut(B, &'p Item) -> B,
    {
        let lock = self.inner.lock().await;
        lock.iter().fold(init, f)
    }
}

pub use display_line::make_line;
use display_line::{highlight, make_hl_ranges};
mod display_line {
    use super::{NvimErr, NvimWtr};
    use nvim_router::nvim_rs::Neovim;
    use nvim_router::nvim_rs::Value;

    use super::{FileType, Item, Level, Metadata};

    use std::ops::Range;
    use std::path::Path;

    pub struct HlRange {
        line: i64,
        col: Range<usize>,
    }
    impl HlRange {
        fn into_value(self, hl: &str) -> Value {
            Value::Map(vec![
                (Value::from("hl"), Value::from(hl)),
                (Value::from("line"), Value::from(self.line)),
                (Value::from("start_col"), Value::from(self.col.start)),
                (Value::from("end_col"), Value::from(self.col.end)),
            ])
        }
    }

    pub struct VirText<T> {
        line: i64,
        text: T,
    }
    fn virt_into_text(line: i64, text: impl Into<Value>, hl: &str) -> Value {
        Value::Map(vec![
            (Value::from("hl"), Value::from(hl)),
            (Value::from("line"), Value::from(line)),
            (Value::from("text"), text.into()),
        ])
    }

    pub enum Highlight {
        Directory(HlRange),
        Regular(HlRange),
        Exec(HlRange),
        NoRead(HlRange),
        NoExecDir(HlRange),
        OtherFile(HlRange),
        Indent(HlRange),
        LinkTo(VirText<String>),
        Metadata(VirText<[u8; 6]>),
    }

    impl Highlight {
        fn into_value(self) -> Value {
            use Highlight::*;
            match self {
                Directory(range) => range.into_value("directory"),
                Regular(range) => range.into_value("regular"),
                Exec(range) => range.into_value("exec"),
                NoRead(range) => range.into_value("no_read"),
                NoExecDir(range) => range.into_value("no_exec_dir"),
                OtherFile(range) => range.into_value("other_file"),
                Indent(range) => range.into_value("indent"),
                LinkTo(virt) => virt_into_text(virt.line, virt.text, "link_to"),
                Metadata(virt) => virt_into_text(
                    virt.line,
                    unsafe { std::str::from_utf8_unchecked(&virt.text) },
                    "metadata",
                ),
            }
        }

        fn indent(line: i64, col: Range<usize>) -> Self {
            let range = HlRange { line, col };
            Self::Indent(range)
        }

        fn from_item(item: &Item, line: i64, col: Range<usize>) -> Self {
            use Highlight::*;

            let range = HlRange { line, col };
            match item.metadata.file_type {
                FileType::Regular | FileType::LinkRegular => {
                    if !item.metadata.perm.read {
                        NoRead(range)
                    } else if item.metadata.perm.exec {
                        Exec(range)
                    } else {
                        Regular(range)
                    }
                }
                FileType::Directory | FileType::LinkDirectory => {
                    if !item.metadata.perm.read {
                        NoRead(range)
                    } else if item.metadata.perm.exec {
                        Directory(range)
                    } else {
                        NoExecDir(range)
                    }
                }
                FileType::Other | FileType::LinkOther => OtherFile(range),
            }
        }
    }

    fn indent_width(level: Level) -> usize {
        let level = level.to_num();
        if level == 0 {
            0
        } else if level == 1 {
            2
        } else {
            let vert_len = "\u{eb10}".len();
            (3 + vert_len) * (level - 1) + 2
        }
    }

    fn metadata_str(metadata: Metadata) -> [u8; 6] {
        let ftype_byte = match metadata.file_type {
            FileType::Regular | FileType::LinkRegular => b'f',
            FileType::Directory | FileType::LinkDirectory => b'd',
            FileType::Other | FileType::LinkOther => b'-',
        };

        let mut meta_str = [b'[', ftype_byte, b'-', b'-', b'-', b']'];
        if metadata.perm.read {
            meta_str[2] = b'r';
        }
        if metadata.perm.write {
            meta_str[3] = b'w';
        }
        if metadata.perm.exec {
            meta_str[4] = b'x';
        }

        meta_str
    }

    fn make_fname(path: &Path, metadata: Metadata) -> String {
        let icon = match metadata.file_type {
            FileType::Regular | FileType::LinkRegular => '\u{f4a5}',
            FileType::Directory | FileType::LinkDirectory => '\u{f413}',
            FileType::Other | FileType::LinkOther => '\u{f29c}',
        };

        let fname = path.file_name().unwrap_or_default();
        let fname: &Path = fname.as_ref();

        format!("{icon} {}", fname.display())
    }

    pub fn make_line(item: &Item) -> String {
        fn indent_str(level: Level, target: &mut String) {
            let level = level.to_num();
            if level == 0 {
                return;
            }
            target.push_str("  ");
            for _ in 1..level {
                target.push('\u{eb10}');
                target.push_str("   ");
            }
        }

        let &Item {
            level,
            ref path,
            metadata,
        } = item;

        let fname = make_fname(path, metadata);

        let mut ret = String::with_capacity(fname.len() + indent_width(level) + 9);
        indent_str(level, &mut ret);
        ret.push_str(&fname);

        if metadata.is_link() {
            ret.push('@');
        }
        if metadata.is_executable() {
            ret.push('*');
        }
        if metadata.is_dir() {
            ret.push('/');
        }

        ret
    }

    pub(super) fn make_hl_ranges<'l, L>(start_line: i64, items: L) -> Vec<Value>
    where
        L: IntoIterator<Item = &'l Item>,
        L::IntoIter: ExactSizeIterator,
    {
        let items = items.into_iter();
        let mut ranges = Vec::with_capacity(items.len());

        for (i, item) in items.enumerate() {
            let line = start_line + i as i64;

            let indent_end = indent_width(item.level);
            let indt_hl = Highlight::indent(line, 0..indent_end);
            ranges.push(indt_hl.into_value());

            let fname_start = indent_end;
            let fname_end = {
                let fname = make_fname(&item.path, item.metadata);
                fname_start + fname.len()
            };

            let item_hl = Highlight::from_item(item, line, fname_start..fname_end);
            ranges.push(item_hl.into_value());

            let meta_hl = Highlight::Metadata(VirText {
                line,
                text: metadata_str(item.metadata),
            });
            ranges.push(meta_hl.into_value());

            if item.metadata.is_link()
                && let Ok(target) = std::fs::read_link(&item.path)
            {
                let target = format!(" \u{f061} {}", target.display());
                let link_hl = Highlight::LinkTo(VirText { line, text: target });
                ranges.push(link_hl.into_value());
            }
        }

        ranges
    }

    pub(super) async fn highlight(
        nvim: &Neovim<NvimWtr>,
        ranges: Vec<Value>,
    ) -> Result<(), NvimErr> {
        nvim.exec_lua(
            "require('lazy-filer.call_lua').set_highlight(...)",
            vec![Value::Array(ranges)],
        )
        .await?;

        Ok(())
    }
}
