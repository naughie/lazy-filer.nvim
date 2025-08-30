use super::{NvimErr, NvimWtr};
use nvim_router::nvim_rs::Neovim;
use nvim_router::nvim_rs::Value;

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

    pub fn edit<'n>(&self, nvim: &'n Neovim<NvimWtr>) -> Edit<'_, 'n> {
        Edit { inner: self, nvim }
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

pub struct Edit<'a, 'n> {
    inner: &'a Items,
    nvim: &'n Neovim<NvimWtr>,
}

struct BufLines(Vec<Value>);
fn items_to_lua<'l, L>(items: L) -> BufLines
where
    L: IntoIterator<Item = &'l Item>,
{
    fn item_to_lua(item: &Item) -> Value {
        use std::path::Path;

        let fname = item.path.file_name().unwrap_or_default();
        let fname: &Path = fname.as_ref();
        let fname = fname.display().to_string();

        let level = item.level.to_num();

        let mut inner = vec![
            (Value::from("fname"), Value::from(fname)),
            (Value::from("level"), Value::from(level)),
            (Value::from("is_link"), Value::from(item.metadata.is_link())),
            (
                Value::from("is_regular"),
                Value::from(item.metadata.is_regular()),
            ),
            (Value::from("is_dir"), Value::from(item.metadata.is_dir())),
            (Value::from("read"), Value::from(item.metadata.perm.read)),
            (Value::from("write"), Value::from(item.metadata.perm.write)),
            (Value::from("exec"), Value::from(item.metadata.perm.exec)),
        ];

        if item.metadata.is_link()
            && let Ok(target) = std::fs::read_link(&item.path)
        {
            let target = target.display().to_string();
            inner.push((Value::from("link_to"), Value::from(target)));
        }

        Value::Map(inner)
    }

    BufLines(items.into_iter().map(item_to_lua).collect())
}

async fn update_buf(
    nvim: &Neovim<NvimWtr>,
    start: i64,
    end: i64,
    BufLines(items): BufLines,
) -> Result<(), NvimErr> {
    nvim.exec_lua(
        "require('lazy-filer.call_lua').update_filer_buf(...)",
        vec![Value::from(start), Value::from(end), Value::Array(items)],
    )
    .await?;

    Ok(())
}

impl Edit<'_, '_> {
    pub async fn replace_all(self, lines: impl Stream<Item = Item>) -> Result<(), NvimErr> {
        let lines = lines.collect::<Vec<_>>().await;
        let items = items_to_lua(&lines);

        let mut lock = self.inner.lock().await;
        *lock = lines;
        drop(lock);

        update_buf(self.nvim, 0, -1, items).await?;

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
        let items = items_to_lua(&lines);

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

        lock.splice(start..end, lines);
        drop(lock);

        update_buf(self.nvim, start as i64, end as i64, items).await?;

        Ok(())
    }

    pub async fn insert(self, lines: impl Stream<Item = Item>, at: LineIdx) -> Result<(), NvimErr> {
        let lines = lines.collect::<Vec<_>>().await;
        let items = items_to_lua(&lines);

        let mut lock = self.inner.lock().await;
        if let Some(at) = at.as_usize(lock.len()) {
            lock.splice(at..at, lines);
        }
        drop(lock);

        let LineIdx(at) = at;
        update_buf(self.nvim, at, at, items).await?;

        Ok(())
    }

    pub async fn insert_dyn(
        self,
        item: Item,
        at: impl for<'a> FnOnce(&'a [Item]) -> usize,
    ) -> Result<(), NvimErr> {
        let mut lock = self.inner.lock().await;

        let at = at(&lock);

        let items = items_to_lua([&item]);

        if at >= lock.len() {
            lock.push(item);
        } else {
            lock.insert(at, item);
        }
        drop(lock);
        update_buf(self.nvim, at as i64, at as i64, items).await?;

        Ok(())
    }

    pub async fn remove(self, at: LineIdx) -> Result<(), NvimErr> {
        let mut lock = self.inner.lock().await;

        if let Some(at) = at.as_usize(lock.len()) {
            lock.remove(at);
        }
        drop(lock);

        let LineIdx(at) = at;
        update_buf(self.nvim, at, at + 1, items_to_lua([])).await?;

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

        update_buf(self.nvim, start as i64, end as i64, items_to_lua([])).await?;

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
