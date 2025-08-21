use super::{NvimErr, NvimWtr};
use nvim_rs::{Buffer, Neovim};

use super::renderer::LineIdx;
use super::utils;
use super::{Action, States};

use std::path::PathBuf;

pub struct DeleteEntry {
    pub line_idx: LineIdx,
    pub nvim: Neovim<NvimWtr>,
    pub buf: Buffer<NvimWtr>,
}

impl Action for DeleteEntry {
    type Resp = ();

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let Some((is_link, entry)) = states
            .actions
            .rendered_lines
            .get(self.line_idx)
            .and_then(|item| {
                item.path.parent().map(|parent| {
                    let path = item.path.to_path_buf();
                    let parent = parent.to_path_buf();
                    let is_link = item.metadata.is_link();

                    if item.metadata.is_dir() {
                        (is_link, Entry::Recursive { parent, path })
                    } else {
                        (is_link, Entry::Single { parent, path })
                    }
                })
            })
            .await
        else {
            return Ok(());
        };

        match entry {
            Entry::Recursive { parent, path } => {
                let target_dir = utils::get_entries(&states.root_file, &parent).await;
                if target_dir.remove_fs(&path, !is_link).await.is_err() {
                    return Ok(());
                }

                states.actions.expanded_dir.remove(&path).await;

                states
                    .actions
                    .rendered_lines
                    .edit(&self.nvim, &self.buf)
                    .remove_range(|lines| utils::find_in_dir(&path, lines))
                    .await?;
            }
            Entry::Single { parent, path } => {
                let target_dir = utils::get_entries(&states.root_file, &parent).await;
                if target_dir.remove_fs(&path, false).await.is_err() {
                    return Ok(());
                }

                states
                    .actions
                    .rendered_lines
                    .edit(&self.nvim, &self.buf)
                    .remove(self.line_idx)
                    .await?;
            }
        }

        Ok(())
    }
}

enum Entry {
    Recursive { parent: PathBuf, path: PathBuf },
    Single { parent: PathBuf, path: PathBuf },
}
