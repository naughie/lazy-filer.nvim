use super::{NvimErr, NvimWtr};
use nvim_router::nvim_rs::Neovim;

use super::{Action, DirArg, States};

use super::utils;

pub struct Refresh {
    pub nvim: Neovim<NvimWtr>,
    pub dir: DirArg,
}

impl Action for Refresh {
    type Resp = ();

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let dir = self.dir.as_path();

        states.actions.expanded_dir.insert(dir.to_path_buf()).await;
        let expanded_dir = states.actions.expanded_dir.clone().await;

        let target_dir = utils::get_entries(&states.root_file, dir).await;

        target_dir
            .update_with_readdir_recursive(&expanded_dir)
            .await?;

        target_dir
            .render_entire_buffer(&self.nvim, &states.actions.rendered_lines, &expanded_dir)
            .await?;

        Ok(())
    }
}
