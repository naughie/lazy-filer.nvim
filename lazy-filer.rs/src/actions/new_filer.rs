use super::{NvimErr, NvimWtr};
use nvim_rs::{Buffer, Neovim};

use super::{Action, DirArg, States};

use super::item::Level;
use super::states::Items;
use super::utils::{self, Entries};
use crate::fs::File;

use std::collections::BTreeSet;
use std::path::PathBuf;

use futures::StreamExt as _;

pub struct NewFiler {
    pub buf: Buffer<NvimWtr>,
    pub dir: DirArg,
    pub nvim: Neovim<NvimWtr>,
}

impl Action for NewFiler {
    type Resp = ();

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let expanded_dir = states.actions.expanded_dir.clone().await;

        let dir = self.dir.as_path();

        let target_dir = utils::get_entries(&states.root_file, dir).await;
        target_dir.update_with_readdir().await?;

        render_new(
            &self.nvim,
            &self.buf,
            &target_dir,
            &states.actions.rendered_lines,
            &expanded_dir,
        )
        .await?;

        for (path, child) in target_dir.children().await {
            if let File::Directory { entries, perm: _ } = child {
                entries.update_with_readdir(&path).await.ok();
            }
        }

        rerender(
            &self.buf,
            &target_dir,
            &states.actions.rendered_lines,
            &expanded_dir,
        )
        .await?;

        Ok(())
    }
}

async fn render_impl(
    buf: &Buffer<NvimWtr>,
    entries: &Entries<'_>,
    lines: &Items,
    expanded_dir: &BTreeSet<PathBuf>,
) -> Result<(), NvimErr> {
    let stream = entries
        .flatten(Level::base())
        .filter(|path| expanded_dir.contains(path))
        .await;

    let recursive = stream.collect::<Vec<_>>().await;

    lines.replace(recursive.iter()).await;

    let lines = recursive.iter().map(utils::make_line).collect();

    buf.set_lines(0, -1, false, lines).await?;

    Ok(())
}

async fn render_new(
    nvim: &Neovim<NvimWtr>,
    buf: &Buffer<NvimWtr>,
    entries: &Entries<'_>,
    lines: &Items,
    expanded_dir: &BTreeSet<PathBuf>,
) -> Result<(), NvimErr> {
    render_impl(buf, entries, lines, expanded_dir).await?;

    nvim.exec_lua("require('lazy-filer').rpc.open_filer_win()", vec![])
        .await?;

    Ok(())
}

async fn rerender(
    buf: &Buffer<NvimWtr>,
    entries: &Entries<'_>,
    lines: &Items,
    expanded_dir: &BTreeSet<PathBuf>,
) -> Result<(), NvimErr> {
    render_impl(buf, entries, lines, expanded_dir).await?;

    Ok(())
}
