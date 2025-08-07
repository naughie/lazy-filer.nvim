use super::{NvimErr, NvimWtr};
use nvim_rs::Buffer;

use super::renderer::{FileType, Item, Items, Level, Metadata};
use crate::fs::{self, File, Permissions, RootFile};

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io::Error as IoErr;
use std::ops::Range;
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

    pub async fn remove_fs(&self, path: &Path, recursive: bool) -> Result<(), IoErr> {
        let Some(fname) = path.file_name() else {
            return Ok(());
        };

        self.entries.remove(fname).await;

        if recursive {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
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
        let stream = self
            .flatten(Level::base())
            .filter(|path| expanded_dir.contains(path))
            .await;

        lines.edit(buf).replace_all(stream).await?;

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

pub fn find_in_dir(prefix: &Path, lines: &[Item]) -> Range<usize> {
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
        end = idx + 1;
    }

    start..end
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
