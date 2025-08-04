use super::{NvimErr, NvimWtr};
use nvim_rs::{Buffer, Neovim};

use super::{Action, DirArg, States};

use super::utils::{self};
use crate::fs::File;

pub struct NewFiler {
    pub buf: Buffer<NvimWtr>,
    pub dir: DirArg,
    pub nvim: Neovim<NvimWtr>,
}

impl Action for NewFiler {
    type Resp = ();

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let dir = self.dir.as_path();

        states.actions.expanded_dir.insert(dir.to_path_buf()).await;
        let expanded_dir = states.actions.expanded_dir.clone().await;

        let target_dir = utils::get_entries(&states.root_file, dir).await;
        target_dir.update_with_readdir().await?;

        target_dir
            .render_entire_buffer(&self.buf, &states.actions.rendered_lines, &expanded_dir)
            .await?;
        open_filer_win(&self.nvim).await?;

        for (path, child) in target_dir.children().await {
            if let File::Directory { entries, perm: _ } = child {
                entries.update_with_readdir(&path).await.ok();
            }
        }

        target_dir
            .render_entire_buffer(&self.buf, &states.actions.rendered_lines, &expanded_dir)
            .await?;

        Ok(())
    }
}

async fn open_filer_win(nvim: &Neovim<NvimWtr>) -> Result<(), NvimErr> {
    nvim.exec_lua("require('lazy-filer').rpc.open_filer_win()", vec![])
        .await?;

    Ok(())
}
