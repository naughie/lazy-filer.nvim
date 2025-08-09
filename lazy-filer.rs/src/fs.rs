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
pub struct Permissions(u32);

impl Permissions {
    fn from_std(perm: std::fs::Permissions) -> Self {
        use std::os::unix::fs::PermissionsExt;
        Self(perm.mode())
    }

    pub fn to_s(self) -> [u8; 3] {
        let mut bytes = [b'-', b'-', b'-'];
        if self.0 & 0o400 != 0 {
            bytes[0] = b'r';
        }
        if self.0 & 0o200 != 0 {
            bytes[1] = b'w';
        }
        if self.0 & 0o100 != 0 {
            bytes[2] = b'x';
        }
        bytes
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
                let perm = Permissions::from_std(metadata.permissions());
                File::Regular { perm }
            } else if file_type.is_dir() {
                let metadata = entry.metadata()?;
                let perm = Permissions::from_std(metadata.permissions());

                File::Directory {
                    entries: Default::default(),
                    perm,
                }
            } else if file_type.is_symlink() {
                let path = entry.path();
                if path.is_file() {
                    let metadata = path.metadata()?;
                    let perm = Permissions::from_std(metadata.permissions());
                    File::Regular { perm }
                } else if path.is_dir() {
                    let metadata = path.metadata()?;
                    let perm = Permissions::from_std(metadata.permissions());
                    File::Directory {
                        entries: Default::default(),
                        perm,
                    }
                } else {
                    File::Other
                }
            } else {
                File::Other
            };

            new_entries.push((fname, file));
        }

        let new_keys: BTreeSet<_> = new_entries.iter().map(|(k, _)| k).collect();
        let mut lock = self.0.lock().await;
        lock.retain(|k, _| new_keys.contains(k));

        for (key, val) in new_entries {
            lock.entry(key).or_insert(val);
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

    pub fn regular(perm: u32) -> Self {
        Self::Regular {
            perm: Permissions(perm),
        }
    }

    pub fn empty_directory(perm: u32) -> Self {
        Self::Directory {
            entries: Default::default(),
            perm: Permissions(perm),
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
                    .map(|meta| Permissions::from_std(meta.permissions()))
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
