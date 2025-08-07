use super::{NvimErr, NvimWtr};
use nvim_rs::Buffer;

use super::{Action, DirArg, States};

use super::utils;

pub struct MoveToParent {
    pub buf: Buffer<NvimWtr>,
    pub dir: DirArg,
}

impl Action for MoveToParent {
    type Resp = ();

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let dir = self.dir.as_path();
        let Some(parent) = dir.parent() else {
            return Ok(());
        };
        states
            .actions
            .expanded_dir
            .insert(parent.to_path_buf())
            .await;
        let expanded_dir = states.actions.expanded_dir.clone().await;

        let target_dir = utils::get_entries(&states.root_file, parent).await;
        target_dir.update_with_readdir().await?;

        target_dir
            .render_entire_buffer(&self.buf, &states.actions.rendered_lines, &expanded_dir)
            .await?;

        Ok(())
    }
}
