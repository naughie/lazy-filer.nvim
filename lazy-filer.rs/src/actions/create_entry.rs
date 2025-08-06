use super::{NvimErr, NvimWtr};
use nvim_rs::Buffer;

use super::item::Level;
use super::states::Items;
use super::utils;
use super::{Action, States};
use crate::fs::File;

use std::ffi::OsStr;
use std::fs::{DirBuilder, OpenOptions};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

pub struct CreateEntry {
    pub line_idx: i64,
    pub buf: Buffer<NvimWtr>,
    pub fname: String,
}

impl Action for CreateEntry {
    type Resp = ();

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let fname = safe_fname(&self.fname);

        let Some(entry) = utils::get_path_at(self.line_idx, &states.actions.rendered_lines)
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
    buf: &Buffer<NvimWtr>,
    lines: &Items,
    path: &Path,
    level: Level,
    file: &File,
) -> Result<(), NvimErr> {
    let mut lock = lines.lock().await;

    let idx = lock
        .iter()
        .enumerate()
        .find_map(|(i, item)| if item.path < path { None } else { Some(i) })
        .unwrap_or_default();

    let item = utils::file_to_item(level, path, file);
    buf.set_lines(idx as i64, idx as i64, false, vec![utils::make_line(&item)])
        .await?;

    lock.insert(idx, item);
    drop(lock);

    Ok(())
}
