use super::{NvimErr, NvimWtr};
use nvim_rs::Buffer;

use super::renderer::{Level, LineIdx};
use super::utils;
use super::{Action, States};

use std::path::Path;

pub struct ExpandDir {
    pub line_idx: LineIdx,
    pub buf: Buffer<NvimWtr>,
}

impl Action for ExpandDir {
    type Resp = ();

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let Some((level, path)) = states
            .actions
            .rendered_lines
            .get(self.line_idx)
            .and_then(|item| {
                if item.metadata.is_dir() {
                    Some((item.level, item.path.to_path_buf()))
                } else {
                    None
                }
            })
            .await
        else {
            return Ok(());
        };

        expand_dir(self.line_idx, &self.buf, level, &path, states).await?;

        Ok(())
    }
}

pub async fn expand_dir(
    line_idx: LineIdx,
    buf: &Buffer<NvimWtr>,
    level: Level,
    path: &Path,
    states: &States,
) -> Result<(), NvimErr> {
    if states.actions.expanded_dir.contains(path).await {
        states.actions.expanded_dir.remove(path).await;

        states
            .actions
            .rendered_lines
            .edit(buf)
            .remove_range(|lines| {
                let range = utils::find_in_dir(path, lines);
                if range.start == range.end {
                    range
                } else {
                    (range.start + 1)..(range.end)
                }
            })
            .await?;
    } else {
        states.actions.expanded_dir.insert(path.to_path_buf()).await;
        let expanded_dir = states.actions.expanded_dir.clone().await;

        let target_dir = utils::get_entries(&states.root_file, path).await;
        target_dir.update_with_readdir().await?;

        let stream = target_dir
            .flatten(level)
            .filter(|path| expanded_dir.contains(path))
            .await;

        states
            .actions
            .rendered_lines
            .edit(buf)
            .insert(stream, line_idx + 1)
            .await?;
    }

    Ok(())
}
