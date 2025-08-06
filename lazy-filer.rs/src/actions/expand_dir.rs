use super::{NvimErr, NvimWtr};
use nvim_rs::Buffer;

use super::item::Level;
use super::states::Items;
use super::utils;
use super::{Action, States};

use std::ops::RangeInclusive;
use std::path::Path;

use futures::StreamExt as _;

pub struct ExpandDir {
    pub line_idx: i64,
    pub buf: Buffer<NvimWtr>,
}

impl Action for ExpandDir {
    type Resp = ();

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let Some((level, path)) = utils::get_path_at(self.line_idx, &states.actions.rendered_lines)
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

async fn remove_items_in(prefix: &Path, lines: &Items) -> RangeInclusive<usize> {
    let mut lock = lines.lock().await;

    let range = utils::find_in_dir(prefix, &lock);
    lock.drain(range.clone());

    range
}

pub async fn expand_dir(
    line_idx: i64,
    buf: &Buffer<NvimWtr>,
    level: Level,
    path: &Path,
    states: &States,
) -> Result<(), NvimErr> {
    if states.actions.expanded_dir.contains(path).await {
        states.actions.expanded_dir.remove(path).await;

        let range = remove_items_in(path, &states.actions.rendered_lines).await;

        buf.set_lines(
            *range.start() as i64,
            *range.end() as i64 + 1,
            false,
            vec![],
        )
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

        let recursive = stream.collect::<Vec<_>>().await;

        let lines = recursive.iter().map(utils::make_line).collect();
        buf.set_lines(line_idx + 1, line_idx + 1, false, lines)
            .await?;

        utils::get_path_at(line_idx, &states.actions.rendered_lines)
            .splice(recursive.into_iter())
            .await;
    }

    Ok(())
}
