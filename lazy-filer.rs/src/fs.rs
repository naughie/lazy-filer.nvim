use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::io::Error as IoError;
use std::path::{Path, PathBuf};

use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Component(OsString);

impl Component {
    pub fn from_slice(s: &OsStr) -> Self {
        Self(s.to_os_string())
    }
}

impl Borrow<OsStr> for Component {
    fn borrow(&self) -> &OsStr {
        &self.0
    }
}
impl AsRef<Path> for Component {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Permissions {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}

impl Permissions {
    fn from_std(meta: std::fs::Metadata) -> Self {
        use nix::unistd;
        use std::os::unix::fs::MetadataExt;

        let mode = meta.mode();
        let file_uid = meta.uid();
        let eff_uid = unistd::geteuid().as_raw();

        if file_uid == eff_uid {
            Self {
                read: mode & 0o400 != 0,
                write: mode & 0o200 != 0,
                exec: mode & 0o100 != 0,
            }
        } else {
            let file_gid = meta.gid();
            let eff_gid = unistd::getegid().as_raw();

            if file_gid == eff_gid {
                Self {
                    read: mode & 0o040 != 0,
                    write: mode & 0o020 != 0,
                    exec: mode & 0o010 != 0,
                }
            } else if let Ok(groups) = unistd::getgroups()
                && groups.iter().any(|gid| gid.as_raw() == file_gid)
            {
                Self {
                    read: mode & 0o040 != 0,
                    write: mode & 0o020 != 0,
                    exec: mode & 0o010 != 0,
                }
            } else {
                Self {
                    read: mode & 0o004 != 0,
                    write: mode & 0o002 != 0,
                    exec: mode & 0o001 != 0,
                }
            }
        }
    }

    fn from_raw(perm: u32) -> Self {
        Self {
            read: perm & 0o400 != 0,
            write: perm & 0o200 != 0,
            exec: perm & 0o100 != 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Entries(Arc<Mutex<BTreeMap<Component, File>>>);

pub struct ChildrenIntoIter<'a>(MutexGuard<'a, BTreeMap<Component, File>>);
impl ChildrenIntoIter<'_> {
    pub fn iter(&self) -> impl Iterator<Item = (&Component, File)> + '_ {
        self.0.iter().map(|(k, v)| (k, v.clone()))
    }
}

impl Entries {
    pub async fn get<Q>(&self, key: &Q) -> Option<File>
    where
        Component: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.0.lock().await.get(key).cloned()
    }

    pub async fn remove<Q>(&self, key: &Q) -> Option<File>
    where
        Component: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.0.lock().await.remove(key)
    }

    pub async fn insert(&self, key: Component, val: File) {
        self.0.lock().await.insert(key, val);
    }

    pub async fn clear(&self) -> BTreeMap<Component, File> {
        let mut lock = self.0.lock().await;
        std::mem::take(&mut lock)
    }

    pub async fn children(&self) -> ChildrenIntoIter<'_> {
        ChildrenIntoIter(self.0.lock().await)
    }

    pub async fn update_with_readdir(&self, dir: &Path) -> Result<(), IoError> {
        let mut new_entries: Vec<(Component, File)> = Default::default();

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;

            let fname = Component(entry.file_name());

            let file = if file_type.is_file() {
                let metadata = entry.metadata()?;
                let perm = Permissions::from_std(metadata);
                File::Regular { perm }
            } else if file_type.is_dir() {
                let metadata = entry.metadata()?;
                let perm = Permissions::from_std(metadata);

                File::Directory {
                    entries: Default::default(),
                    perm,
                }
            } else if file_type.is_symlink() {
                let path = entry.path();
                let file = if path.is_file() {
                    let metadata = path.metadata()?;
                    let perm = Permissions::from_std(metadata);
                    File::Regular { perm }
                } else if path.is_dir() {
                    let metadata = path.metadata()?;
                    let perm = Permissions::from_std(metadata);
                    File::Directory {
                        entries: Default::default(),
                        perm,
                    }
                } else {
                    File::Other
                };
                File::Link { to: Box::new(file) }
            } else {
                File::Other
            };

            new_entries.push((fname, file));
        }

        let new_keys: BTreeSet<_> = new_entries.iter().map(|(k, _)| k).collect();
        let mut lock = self.0.lock().await;
        lock.retain(|k, _| new_keys.contains(k));

        for (key, new_file) in new_entries {
            if let Some(old_file) = lock.get_mut(&key) {
                match (old_file, new_file) {
                    (
                        File::Directory { perm, entries: _ },
                        File::Directory {
                            perm: new_perm,
                            entries: _,
                        },
                    ) => {
                        *perm = new_perm;
                    }
                    (File::Link { to: old_to }, File::Link { to: new_to }) => {
                        match (old_to.follow_link_mut(), new_to.follow_link_owned()) {
                            (
                                File::Directory { perm, entries: _ },
                                File::Directory {
                                    perm: new_perm,
                                    entries: _,
                                },
                            ) => {
                                *perm = new_perm;
                            }
                            (old_to, new_to) => *old_to = new_to,
                        }
                    }
                    (old_file, new_file) => {
                        *old_file = new_file;
                    }
                }
            } else {
                lock.insert(key, new_file);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum File {
    Regular { perm: Permissions },
    Directory { entries: Entries, perm: Permissions },
    Link { to: Box<File> },
    Other,
}

impl File {
    pub fn follow_link(&self) -> &Self {
        let mut ret = self;
        loop {
            match ret {
                File::Link { to } => ret = to,
                _ => return ret,
            }
        }
    }

    pub fn follow_link_mut(&mut self) -> &mut Self {
        let mut ret = self;
        loop {
            match ret {
                File::Link { to } => ret = to,
                _ => return ret,
            }
        }
    }

    pub fn follow_link_owned(self) -> Self {
        let mut ret = self;
        loop {
            match ret {
                File::Link { to } => ret = *to,
                _ => return ret,
            }
        }
    }

    pub fn regular(perm: u32) -> Self {
        Self::Regular {
            perm: Permissions::from_raw(perm),
        }
    }

    pub fn empty_directory(perm: u32) -> Self {
        Self::Directory {
            entries: Default::default(),
            perm: Permissions::from_raw(perm),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RootFile {
    entries: Entries,
}

impl RootFile {
    pub async fn get_entries(&self, dir: &Path) -> Entries {
        let mut components = dir.iter();

        {
            let first = components.next();
            assert_eq!(first, Some(OsStr::new("/")));
        }

        let mut stack = PathBuf::from("/");
        let mut entries = self.entries.clone();

        for component in components {
            stack.push(component);
            let is_symlink = stack.is_symlink();

            if let Some(next) = entries.get(component).await {
                match next.follow_link() {
                    File::Directory {
                        entries: next,
                        perm: _,
                    } => entries = next.clone(),
                    _ => unimplemented!(),
                }
            } else {
                let perm = stack
                    .metadata()
                    .map(Permissions::from_std)
                    .unwrap_or_default();
                let next = Entries::default();
                let real_dir = File::Directory {
                    entries: next.clone(),
                    perm,
                };

                if is_symlink {
                    let file = File::Link {
                        to: Box::new(real_dir),
                    };

                    entries.insert(Component::from_slice(component), file).await;
                } else {
                    entries
                        .insert(Component::from_slice(component), real_dir)
                        .await;
                }

                entries = next;
            }
        }

        entries
    }
}
