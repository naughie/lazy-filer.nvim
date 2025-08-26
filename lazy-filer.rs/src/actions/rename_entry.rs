use super::{NvimErr, NvimWtr};
use nvim_router::nvim_rs::Neovim;

use super::renderer::{Level, LineIdx};
use super::utils;
use super::{Action, DirArg, States};

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub struct RenameEntry {
    pub line_idx: LineIdx,
    pub nvim: Neovim<NvimWtr>,
    pub dir: DirArg,
    pub path: String,
}

impl Action for RenameEntry {
    type Resp = ();

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let Some(old_path) = states
            .actions
            .rendered_lines
            .get(self.line_idx)
            .and_then(|item| Some(item.path.to_path_buf()))
            .await
        else {
            return Ok(());
        };

        let Some(old_parent) = old_path.parent() else {
            return Ok(());
        };

        let new_path = resolve(&old_path, self.path.as_ref());
        let Some(new_parent) = new_path.parent() else {
            return Ok(());
        };

        let new_fname = safe_fname(&new_path);

        if std::fs::rename(&old_path, &new_path).is_err() {
            return Ok(());
        }

        let src_dir = utils::get_entries(&states.root_file, old_parent).await;
        let Some(file) = src_dir.remove(&old_path).await else {
            return Ok(());
        };
        let dst_dir = utils::get_entries(&states.root_file, new_parent).await;
        dst_dir.insert(new_fname, file).await;

        let dir = self.dir.as_path();
        let ancestor = if is_common_ancestor(dir, &old_path, &new_path) {
            let ancestor = states
                .actions
                .rendered_lines
                .iter()
                .fold((Level::base(), dir.to_path_buf()), |acc, item| {
                    let dir = &item.path;
                    if is_common_ancestor(dir, &old_path, &new_path) {
                        if dir.starts_with(&acc.1) {
                            (item.level, dir.to_path_buf())
                        } else {
                            acc
                        }
                    } else {
                        acc
                    }
                })
                .await;
            Some(ancestor)
        } else {
            None
        };

        states
            .actions
            .expanded_dir
            .lock()
            .then(|expanded_dir| {
                let expanded = expanded_dir.remove(&old_path);

                let Some((_, ancestor)) = ancestor.as_ref() else {
                    return;
                };

                let ancestor = ancestor.to_path_buf();

                if expanded {
                    expanded_dir.insert(new_path.clone());
                }

                let mut path = new_path.clone();
                while path.pop() && path.starts_with(&ancestor) {
                    expanded_dir.insert(path.to_path_buf());
                }
            })
            .await;

        if let Some((level, ancestor)) = ancestor {
            let expanded_dir = states.actions.expanded_dir.clone().await;
            let target_dir = utils::get_entries(&states.root_file, &ancestor).await;

            let stream = target_dir
                .flatten(level)
                .filter(|path| expanded_dir.contains(path))
                .await;

            states
                .actions
                .rendered_lines
                .edit(&self.nvim)
                .replace_range(stream, |lines| {
                    let range = utils::find_in_dir(&ancestor, lines);
                    if range.start == range.end || level == Level::base() {
                        range
                    } else {
                        (range.start + 1)..(range.end)
                    }
                })
                .await?;
        } else {
            states
                .actions
                .rendered_lines
                .edit(&self.nvim)
                .remove_range(|lines| utils::find_in_dir(&old_path, lines))
                .await?;
        }

        Ok(())
    }
}

fn safe_fname(path: &Path) -> &OsStr {
    path.file_name().unwrap_or_default()
}

fn resolve(old_path: &Path, new_path: &Path) -> PathBuf {
    fn concat(old_path: &Path, new_path: &Path) -> PathBuf {
        use std::path::Component;

        if new_path.is_absolute() {
            return new_path.to_path_buf();
        }

        let mut ret = old_path.to_path_buf();
        ret.pop();

        for component in new_path.components() {
            match component {
                Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
                Component::ParentDir => {
                    ret.pop();
                }
                Component::Normal(fname) => {
                    ret.push(fname);
                }
            }
        }

        ret
    }

    fn append_fname(old_path: &Path, mut new_path: PathBuf) -> PathBuf {
        if new_path.is_dir()
            && let Some(fname) = old_path.file_name()
        {
            new_path.push(fname);
        }
        new_path
    }

    let ret = concat(old_path, new_path);
    append_fname(old_path, ret)
}

fn is_common_ancestor(anc: &Path, this: &Path, that: &Path) -> bool {
    this.starts_with(anc) && that.starts_with(anc)
}
