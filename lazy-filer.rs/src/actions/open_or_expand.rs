use super::{NvimErr, NvimWtr};
use nvim_router::nvim_rs::Value;
use nvim_router::nvim_rs::{Buffer, Neovim};

use super::renderer::{Level, LineIdx};
use super::{Action, States};
use super::{expand_dir::expand_dir, open_file::open_file};

use std::path::PathBuf;

pub struct OpenOrExpand {
    pub line_idx: LineIdx,
    pub nvim: Neovim<NvimWtr>,
    pub buf: Buffer<NvimWtr>,
}

#[derive(Debug)]
enum Path {
    Regular(Value),
    Directory(Level, PathBuf),
}

impl Action for OpenOrExpand {
    type Resp = ();

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let Some(path) = states
            .actions
            .rendered_lines
            .get(self.line_idx)
            .and_then(|item| {
                if item.metadata.is_regular() {
                    item.path.to_str().map(Value::from).map(Path::Regular)
                } else if item.metadata.is_dir() {
                    Some(Path::Directory(item.level, item.path.to_path_buf()))
                } else {
                    None
                }
            })
            .await
        else {
            return Ok(());
        };

        match path {
            Path::Regular(path) => open_file(&self.nvim, path).await?,
            Path::Directory(level, path) => {
                expand_dir(self.line_idx, &self.nvim, &self.buf, level, &path, states).await?
            }
        }

        Ok(())
    }
}
