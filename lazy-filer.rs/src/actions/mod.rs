use super::{NvimErr, NvimWtr};
use crate::states::States;

mod renderer;
mod utils;

mod create_entry;
mod delete_entry;
mod expand_dir;
mod get_dir;
mod get_file_path;
mod move_to_parent;
mod new_filer;
mod open_file;
mod open_or_expand;
mod refresh;
mod rename_entry;

use std::future::Future;
use std::path::Path;

pub struct DirArg(String);

impl DirArg {
    fn as_path(&self) -> &Path {
        self.0.as_ref()
    }
}

impl From<String> for DirArg {
    fn from(value: String) -> Self {
        Self(value)
    }
}

pub trait Action {
    type Resp;

    fn run(&self, states: &States) -> impl Future<Output = Result<Self::Resp, NvimErr>>;
}

pub mod prelude {
    pub use super::Action;
    pub use super::{
        create_entry::CreateEntry, delete_entry::DeleteEntry, expand_dir::ExpandDir,
        get_dir::GetDir, get_file_path::GetFilePath, move_to_parent::MoveToParent,
        new_filer::NewFiler, open_file::OpenFile, open_or_expand::OpenOrExpand, refresh::Refresh,
        rename_entry::RenameEntry,
    };
}

pub mod states {
    use super::renderer::Items;

    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Debug, Clone, Default)]
    pub struct States {
        pub rendered_lines: Items,
        pub expanded_dir: ExpendedDir,
    }

    #[derive(Debug, Clone, Default)]
    pub struct ExpendedDir(Arc<Mutex<BTreeSet<PathBuf>>>);

    impl ExpendedDir {
        pub async fn clone(&self) -> BTreeSet<PathBuf> {
            let lock = self.0.lock().await;
            lock.clone()
        }

        pub async fn contains(&self, path: &Path) -> bool {
            let lock = self.0.lock().await;
            lock.contains(path)
        }

        pub async fn insert(&self, path: PathBuf) {
            let mut lock = self.0.lock().await;
            lock.insert(path);
        }

        pub async fn remove(&self, path: &Path) -> bool {
            let mut lock = self.0.lock().await;
            lock.remove(path)
        }

        pub fn lock(&self) -> ExpendedDirLock<'_> {
            ExpendedDirLock(self)
        }
    }

    pub struct ExpendedDirLock<'a>(&'a ExpendedDir);

    impl ExpendedDirLock<'_> {
        pub async fn then<Func, T>(self, f: Func) -> T
        where
            Func: for<'b> FnOnce(&'b mut BTreeSet<PathBuf>) -> T,
        {
            let mut lock = self.0.0.lock().await;
            f(&mut lock)
        }
    }
}
