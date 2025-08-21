use super::{NvimErr, NvimWtr};
use nvim_rs::{Buffer, Neovim};

use super::renderer::{Items, Level, LineIdx};
use super::utils;
use super::{Action, States};
use crate::fs::File;

use std::ffi::OsStr;
use std::fs::{DirBuilder, OpenOptions};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

pub struct CreateEntry {
    pub line_idx: LineIdx,
    pub nvim: Neovim<NvimWtr>,
    pub buf: Buffer<NvimWtr>,
    pub fname: String,
}

impl Action for CreateEntry {
    type Resp = ();

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let fname = safe_fname(&self.fname);

        let Some(entry) = states
            .actions
            .rendered_lines
            .get(self.line_idx)
            .and_then(|item| {
                let dir = if item.metadata.is_dir() {
                    Some((&*item.path, item.level.increment()))
                } else {
                    item.path.parent().map(|dir| (dir, item.level))
                };

                dir.map(|(dir, level)| Entry {
                    dir: dir.to_path_buf(),
                    file: dir.join(fname),
                    level,
                })
            })
            .await
        else {
            return Ok(());
        };

        let file = if self.fname.ends_with('/') {
            let perm = 0o775;
            let mut builder = DirBuilder::new();
            builder.mode(perm);

            if builder.create(&entry.file).is_err() {
                return Ok(());
            }
            File::empty_directory(perm)
        } else {
            let perm = 0o664;
            let mut opts = OpenOptions::new();
            opts.write(true).create(true).mode(perm);
            if opts.open(&entry.file).is_err() {
                return Ok(());
            }

            File::regular(perm)
        };

        insert(
            &self.nvim,
            &self.buf,
            &states.actions.rendered_lines,
            &entry.file,
            entry.level,
            &file,
        )
        .await?;

        states.actions.expanded_dir.insert(entry.dir.clone()).await;

        let target_dir = utils::get_entries(&states.root_file, &entry.dir).await;
        target_dir.insert(fname, file).await;

        Ok(())
    }
}

fn safe_fname(s: &str) -> &OsStr {
    let path: &Path = s.as_ref();
    path.file_name().unwrap_or_default()
}

struct Entry {
    dir: PathBuf,
    file: PathBuf,
    level: Level,
}

async fn insert(
    nvim: &Neovim<NvimWtr>,
    buf: &Buffer<NvimWtr>,
    lines: &Items,
    path: &Path,
    level: Level,
    file: &File,
) -> Result<(), NvimErr> {
    lines
        .edit(nvim, buf)
        .insert_dyn(utils::file_to_item(level, path, file), |lines| {
            lines
                .iter()
                .enumerate()
                .find_map(|(i, item)| if item.path < path { None } else { Some(i) })
                .unwrap_or_default()
        })
        .await?;

    Ok(())
}
