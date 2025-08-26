use super::{NvimErr, NvimWtr};
use nvim_router::nvim_rs::Neovim;

use super::renderer::{FileType, Item, Items, Level, Metadata};
use crate::fs::{self, File, Permissions, RootFile};

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io::Error as IoErr;
use std::marker::PhantomData;
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

async fn update_with_readdir(entries: &fs::Entries, dir: &Path) -> Result<(), NvimErr> {
    use nvim_router::nvim_rs::error::CallError;

    if let Err(e) = entries.update_with_readdir(dir).await {
        let msg = e.to_string();
        Err(Box::new(CallError::NeovimError(Some(0), msg)))
    } else {
        Ok(())
    }
}

impl<'a> Entries<'a> {
    pub async fn update_with_readdir(&self) -> Result<(), NvimErr> {
        update_with_readdir(&self.entries, self.dir).await
    }

    pub async fn update_with_readdir_recursive(
        &self,
        expanded_dir: &BTreeSet<PathBuf>,
    ) -> Result<(), NvimErr> {
        let filter = |path: &Path| expanded_dir.contains(path);

        let mut stack = {
            let children = self.children().await;
            let level = Level::base();
            vec![(level.increment(), children.into_iter())]
        };

        while let Some(&mut (level, ref mut children)) = stack.last_mut() {
            let Some((child_path, child)) = children.next() else {
                stack.pop();
                continue;
            };

            match child {
                File::Directory { entries, perm: _ } => {
                    update_with_readdir(&entries, &child_path).await?;

                    if filter(&child_path) && level < Level::MAX {
                        let mut children = Entries::children_in(&entries, &child_path).await;
                        children.sort();
                        stack.push((level.increment(), children.into_iter()));
                    }
                }
                File::Link { to } => {
                    let file = to.follow_link();

                    if let File::Directory { entries, perm: _ } = file {
                        update_with_readdir(entries, &child_path).await?;

                        if filter(&child_path) && level < Level::MAX {
                            let mut children = Entries::children_in(entries, &child_path).await;
                            children.sort();
                            stack.push((level.increment(), children.into_iter()));
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub async fn remove(&self, path: &Path) -> Option<File> {
        let fname = path.file_name()?;
        self.entries.remove(fname).await
    }

    pub async fn remove_fs(&self, path: &Path, recursive: bool) -> Result<(), IoErr> {
        async fn remove_recursive(file: File) {
            let mut stack = match file {
                File::Directory { perm: _, entries } => vec![entries],
                File::Link { to } => {
                    let file = to.follow_link_owned();
                    match file {
                        File::Directory { perm: _, entries } => vec![entries],
                        _ => return,
                    }
                }
                _ => return,
            };

            while let Some(entries) = stack.pop() {
                let files = entries.clear().await;
                for file in files.into_values() {
                    match file {
                        File::Directory { perm: _, entries } => stack.push(entries),
                        File::Link { to } => {
                            let file = to.follow_link_owned();
                            match file {
                                File::Directory { perm: _, entries } => stack.push(entries),
                                _ => continue,
                            }
                        }
                        _ => continue,
                    }
                }
            }
        }

        let Some(fname) = path.file_name() else {
            return Ok(());
        };

        let file = self.entries.remove(fname).await;

        if recursive {
            if let Some(file) = file {
                remove_recursive(file).await;
            }
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

    pub fn flatten(&self, level: Level) -> FlattenEntries<'a, '_, Item> {
        FlattenEntries {
            inner: self,
            level,
            marker: PhantomData,
        }
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
        nvim: &Neovim<NvimWtr>,
        lines: &Items,
        expanded_dir: &BTreeSet<PathBuf>,
    ) -> Result<(), NvimErr> {
        use futures::stream::{StreamExt as _, once};

        let stream = self
            .flatten(Level::base())
            .filter(|path| expanded_dir.contains(path))
            .await;

        let cwd = once(async {
            Item {
                level: Level::base(),
                path: self.dir.to_path_buf(),
                metadata: Metadata {
                    perm: Permissions::read_from_path(self.dir),
                    file_type: FileType::Directory,
                },
            }
        });

        lines.edit(nvim).replace_all(cwd.chain(stream)).await?;

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

pub struct FlattenEntries<'a, 'e, T> {
    level: Level,
    inner: &'e Entries<'a>,
    marker: PhantomData<T>,
}

impl<'a, 'e, T> FlattenEntries<'a, 'e, T> {
    pub async fn filter<Filt>(self, filter: Filt) -> impl Stream<Item = T>
    where
        Filt: for<'p> Fn(&'p Path) -> bool,
        Item: Into<T>,
    {
        let inner = FlattenFilterEntries {
            inner: self.inner,
            filter,
            marker: PhantomData,
        };
        inner.into_stream(self.level).await
    }
}

struct FlattenFilterEntries<'a, 'e, T, Filt> {
    inner: &'e Entries<'a>,
    filter: Filt,
    marker: PhantomData<T>,
}

impl<T, Filt> FlattenFilterEntries<'_, '_, T, Filt>
where
    Filt: for<'p> Fn(&'p Path) -> bool,
    Item: Into<T>,
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

    async fn into_stream(self, level: Level) -> impl Stream<Item = T> {
        let init = self.iter(level).await;
        futures::stream::unfold(init, |mut state| async {
            let v = state.next_item().await?;
            Some((v.into(), state))
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
                    if (self.filter)(&child_path) && level < Level::MAX {
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
                        &File::Regular { perm } => Metadata {
                            perm,
                            file_type: FileType::LinkRegular,
                        },
                        File::Directory { entries, perm } => {
                            if (self.filter)(&child_path) && level < Level::MAX {
                                let mut children = Entries::children_in(entries, &child_path).await;
                                children.sort();
                                self.stack.push((level.increment(), children.into_iter()));
                            }

                            Metadata {
                                perm: *perm,
                                file_type: FileType::LinkDirectory,
                            }
                        }
                        _ => Metadata {
                            perm: Permissions::default(),
                            file_type: FileType::LinkOther,
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
        .skip_while(|(_, item)| !item.path.starts_with(prefix))
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
                    file_type: FileType::LinkRegular,
                },
                File::Directory { perm, entries: _ } => Metadata {
                    perm,
                    file_type: FileType::LinkDirectory,
                },
                _ => Metadata {
                    perm: Permissions::default(),
                    file_type: FileType::LinkOther,
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
