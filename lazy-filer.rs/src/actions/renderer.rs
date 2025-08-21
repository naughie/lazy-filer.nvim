use super::{NvimErr, NvimWtr};
use nvim_rs::Value;
use nvim_rs::{Buffer, Neovim};

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

    pub fn repeat(self, mut f: impl FnMut()) {
        let level = self.0;
        for _ in 0..level {
            f();
        }
    }

    pub const MAX: Self = Self(10);
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
            Self::Regular => b'f',
            Self::Directory => b'd',
            Self::Other => b'-',
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
        let meta = [ftype, perm[0], perm[1], perm[2]];
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
    fn from_value(n: Value) -> Option<Self> {
        let Value::Integer(line_idx) = n else {
            return None;
        };
        let line_idx = line_idx.as_i64()?;
        Some(Self(line_idx))
    }

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

impl TryFrom<Value> for LineIdx {
    type Error = Value;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Self::from_value(value).ok_or(Value::Nil)
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

fn make_hl_ranges<'l, L>(start_line: i64, items: L) -> Vec<Value>
where
    L: IntoIterator<Item = &'l Item>,
    L::IntoIter: ExactSizeIterator,
{
    fn hl_group(item: &Item) -> &'static str {
        match item.metadata.file_type {
            FileType::Regular => "regular",
            FileType::Directory => "directory",
            FileType::Other => "other_file",
        }
    }

    let items = items.into_iter();
    let mut ranges = Vec::with_capacity(2 * items.len());

    for (i, item) in items.enumerate() {
        let line = start_line + i as i64;

        let offset = item.level.to_num() * 2;

        let meta_range = offset..(offset + 6);
        let fname_start = offset + 7;
        let fname_end = {
            let fname = item.path.file_name().unwrap_or_default();
            fname_start + fname.to_string_lossy().len()
        };

        let meta_range = vec![
            (Value::from("hl"), Value::from("metadata")),
            (Value::from("line"), Value::from(line)),
            (Value::from("start_col"), Value::from(meta_range.start)),
            (Value::from("end_col"), Value::from(meta_range.end)),
        ];
        let fname_range = vec![
            (Value::from("hl"), Value::from(hl_group(item))),
            (Value::from("line"), Value::from(line)),
            (Value::from("start_col"), Value::from(fname_start)),
            (Value::from("end_col"), Value::from(fname_end)),
        ];

        ranges.push(Value::Map(meta_range));
        ranges.push(Value::Map(fname_range));
    }

    ranges
}

async fn highlight(nvim: &Neovim<NvimWtr>, ranges: Vec<Value>) -> Result<(), NvimErr> {
    nvim.exec_lua(
        "require('lazy-filer.call_lua').set_highlight(...)",
        vec![Value::Array(ranges)],
    )
    .await?;

    Ok(())
}

pub fn make_line(item: &Item) -> String {
    let &Item {
        level,
        ref path,
        metadata,
    } = item;

    let fname = path.file_name().unwrap_or_default();

    let mut ret = String::with_capacity(fname.len() + 2 * level.to_num() + 7);
    level.repeat(|| ret.push_str("  "));
    metadata.push(&mut ret);
    ret.push_str(&fname.to_string_lossy());

    if metadata.is_dir() {
        ret.push('/');
    }

    ret
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
