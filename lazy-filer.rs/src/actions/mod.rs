use super::{NvimErr, NvimWtr};
use crate::states::States;

mod item;
mod utils;

mod expand_dir;
mod move_to_parent;
mod new_filer;
mod open_file;
mod open_or_expand;

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
        expand_dir::ExpandDir, move_to_parent::MoveToParent, new_filer::NewFiler,
        open_file::OpenFile, open_or_expand::OpenOrExpand,
    };
}

pub mod states {
    use super::item::Item;

    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::sync::MutexGuard;

    #[derive(Debug, Clone, Default)]
    pub struct States {
        pub rendered_lines: Items,
        pub expanded_dir: ExpendedDir,
    }

    #[derive(Debug, Clone, Default)]
    pub struct Items(Arc<Mutex<Vec<Item>>>);

    impl Items {
        pub async fn lock(&self) -> MutexGuard<'_, Vec<Item>> {
            self.0.lock().await
        }

        pub async fn replace(&self, new_lines: impl Iterator<Item = &Item>) {
            let new_lines = new_lines.cloned().collect();

            let mut lock = self.0.lock().await;
            *lock = new_lines;
        }
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

        pub async fn remove(&self, path: &Path) {
            let mut lock = self.0.lock().await;
            lock.remove(path);
        }
    }
}
