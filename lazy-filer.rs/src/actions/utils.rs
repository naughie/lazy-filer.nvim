use super::{NvimErr, NvimWtr};
use nvim_rs::Buffer;

use super::item::{FileType, Item, Level, Metadata};
use super::states::Items;
use crate::fs::{self, File, Permissions, RootFile};

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use futures::stream::Stream;

pub struct Entries<'a> {
    entries: fs::Entries,
    dir: &'a Path,
}

pub async fn get_entries<'a>(root: &RootFile, dir: &'a Path) -> Entries<'a> {
    let entries = root.get_entries(dir).await;
    Entries { entries, dir }
}

impl<'a> Entries<'a> {
    pub async fn update_with_readdir(&self) -> Result<(), NvimErr> {
        use nvim_rs::error::CallError;

        if let Err(e) = self.entries.update_with_readdir(self.dir).await {
            let msg = e.to_string();
            Err(Box::new(CallError::NeovimError(Some(0), msg)))
        } else {
            Ok(())
        }
    }

    pub async fn insert(&self, fname: &OsStr, file: File) {
        use fs::Component;
        self.entries
            .insert(Component::from_slice(fname), file)
            .await;
    }

    pub fn flatten(&self, level: Level) -> FlattenEntries<'a, '_> {
        FlattenEntries { inner: self, level }
    }

    pub async fn children(&self) -> Children {
        Self::children_in(&self.entries, self.dir).await
    }

    async fn children_in(entries: &fs::Entries, dir: &Path) -> Children {
        let children = entries.children().await;
        Children(children.iter().map(|(k, v)| (dir.join(k), v)).collect())
    }

    pub async fn render_entire_buffer(
        &self,
        buf: &Buffer<NvimWtr>,
        lines: &Items,
        expanded_dir: &BTreeSet<PathBuf>,
    ) -> Result<(), NvimErr> {
        use futures::StreamExt as _;

        let stream = self
            .flatten(Level::base())
            .filter(|path| expanded_dir.contains(path))
            .await;

        let recursive = stream.collect::<Vec<_>>().await;

        lines.replace(recursive.iter()).await;

        let lines = recursive.iter().map(make_line).collect();

        buf.set_lines(0, -1, false, lines).await?;

        Ok(())
    }
}

pub struct Children(Vec<(PathBuf, File)>);

impl IntoIterator for Children {
    type Item = (PathBuf, File);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Children {
    fn sort(&mut self) {
        self.0.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    }
}

pub struct FlattenEntries<'a, 'e> {
    level: Level,
    inner: &'e Entries<'a>,
}

impl<'a, 'e> FlattenEntries<'a, 'e> {
    pub async fn filter<Filt>(self, filter: Filt) -> impl Stream<Item = Item>
    where
        Filt: for<'p> Fn(&'p Path) -> bool,
    {
        let inner = FlattenFilterEntries {
            inner: self.inner,
            filter,
        };
        inner.into_stream(self.level).await
    }
}

struct FlattenFilterEntries<'a, 'e, Filt> {
    inner: &'e Entries<'a>,
    filter: Filt,
}

impl<Filt> FlattenFilterEntries<'_, '_, Filt>
where
    Filt: for<'p> Fn(&'p Path) -> bool,
{
    async fn iter(self, level: Level) -> FlattenEntriesIter<Filt> {
        let mut children = self.inner.children().await;
        children.sort();
        let stack = vec![(level.increment(), children.into_iter())];

        FlattenEntriesIter {
            stack,
            filter: self.filter,
        }
    }

    async fn into_stream(self, level: Level) -> impl Stream<Item = Item> {
        let init = self.iter(level).await;
        futures::stream::unfold(init, |mut state| async {
            let v = state.next_item().await?;
            Some((v, state))
        })
    }
}

struct FlattenEntriesIter<Filt> {
    stack: Vec<(Level, <Children as IntoIterator>::IntoIter)>,
    filter: Filt,
}

impl<Filt> FlattenEntriesIter<Filt>
where
    Filt: for<'p> Fn(&'p Path) -> bool,
{
    async fn next_item(&mut self) -> Option<Item> {
        while let Some(&mut (level, ref mut children)) = self.stack.last_mut() {
            let Some((child_path, child)) = children.next() else {
                self.stack.pop();
                continue;
            };

            let metadata = match child {
                File::Regular { perm } => Metadata {
                    perm,
                    file_type: FileType::Regular,
                },
                File::Directory { entries, perm } => {
                    if (self.filter)(&child_path) {
                        let mut children = Entries::children_in(&entries, &child_path).await;
                        children.sort();
                        self.stack.push((level.increment(), children.into_iter()));
                    }

                    Metadata {
                        perm,
                        file_type: FileType::Directory,
                    }
                }
                File::Link { to } => {
                    let file = to.follow_link();

                    match file {
                        File::Regular { perm } => Metadata {
                            perm: *perm,
                            file_type: FileType::Regular,
                        },
                        File::Directory { entries, perm } => {
                            if (self.filter)(&child_path) {
                                let mut children = Entries::children_in(entries, &child_path).await;
                                children.sort();
                                self.stack.push((level.increment(), children.into_iter()));
                            }

                            Metadata {
                                perm: *perm,
                                file_type: FileType::Directory,
                            }
                        }
                        _ => Metadata {
                            perm: Permissions::default(),
                            file_type: FileType::Other,
                        },
                    }
                }
                _ => Metadata {
                    perm: Permissions::default(),
                    file_type: FileType::Other,
                },
            };

            return Some(Item {
                level,
                path: child_path,
                metadata,
            });
        }

        None
    }
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
    idx: i64,
    lines: &'a Items,
}

pub fn get_path_at(idx: i64, lines: &Items) -> PathGetter<'_> {
    PathGetter { idx, lines }
}

fn idx_as_usize<S>(idx: i64, lines: &[S]) -> Option<usize> {
    if idx >= 0 {
        Some(idx as usize)
    } else {
        let len = lines.len();
        let idx = (len as i64) + idx;
        if idx >= 0 { Some(idx as usize) } else { None }
    }
}

impl PathGetter<'_> {
    pub async fn and_then<Func, T>(self, f: Func) -> Option<T>
    where
        Func: for<'p> FnOnce(&'p Item) -> Option<T>,
    {
        let lock = self.lines.lock().await;
        idx_as_usize(self.idx, &lock)
            .and_then(|idx| lock.get(idx))
            .and_then(f)
    }

    pub async fn splice(self, replacement: impl Iterator<Item = Item>) {
        let mut lock = self.lines.lock().await;
        if let Some(idx) = idx_as_usize(self.idx, &lock) {
            let range = (idx + 1)..(idx + 1);
            lock.splice(range, replacement);
        }
    }
}

pub fn find_in_dir(prefix: &Path, lines: &[Item]) -> RangeInclusive<usize> {
    let mut start = lines.len();
    let mut end = start;

    for idx in lines
        .iter()
        .enumerate()
        .skip_while(|(_, item)| !item.path.starts_with(prefix) || item.path == prefix)
        .map_while(|(idx, item)| {
            if item.path.starts_with(prefix) {
                Some(idx)
            } else {
                None
            }
        })
    {
        if idx < start {
            start = idx;
        }
        end = idx;
    }

    start..=end
}

pub fn file_to_item(level: Level, path: &Path, file: &File) -> Item {
    let metadata = match file {
        &File::Regular { perm } => Metadata {
            perm,
            file_type: FileType::Regular,
        },
        &File::Directory { perm, entries: _ } => Metadata {
            perm,
            file_type: FileType::Directory,
        },
        File::Link { to } => {
            let file = to.follow_link();
            match *file {
                File::Regular { perm } => Metadata {
                    perm,
                    file_type: FileType::Regular,
                },
                File::Directory { perm, entries: _ } => Metadata {
                    perm,
                    file_type: FileType::Directory,
                },
                _ => Metadata {
                    perm: Permissions::default(),
                    file_type: FileType::Other,
                },
            }
        }
        _ => Metadata {
            perm: Permissions::default(),
            file_type: FileType::Other,
        },
    };

    Item {
        level,
        path: path.to_path_buf(),
        metadata,
    }
}
