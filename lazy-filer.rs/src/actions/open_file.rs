use super::{NvimErr, NvimWtr};
use nvim_router::nvim_rs::Neovim;
use nvim_router::nvim_rs::Value;

use super::renderer::LineIdx;
use super::{Action, States};

pub struct OpenFile {
    pub line_idx: LineIdx,
    pub nvim: Neovim<NvimWtr>,
}

impl Action for OpenFile {
    type Resp = ();

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let Some(path) = states
            .actions
            .rendered_lines
            .get(self.line_idx)
            .and_then(|item| {
                if item.metadata.is_regular() {
                    item.path.to_str().map(Value::from)
                } else {
                    None
                }
            })
            .await
        else {
            return Ok(());
        };

        open_file(&self.nvim, path).await?;

        Ok(())
    }
}

async fn fname_escape(nvim: &Neovim<NvimWtr>, path: Value) -> Result<Value, NvimErr> {
    nvim.call_function("fnameescape", vec![path]).await
}

async fn new_edit(nvim: &Neovim<NvimWtr>, path: &str) -> Result<(), NvimErr> {
    let cmd = format!("edit! {path}");
    nvim.command(&cmd).await?;

    Ok(())
}

pub async fn open_file(nvim: &Neovim<NvimWtr>, path: Value) -> Result<(), NvimErr> {
    let path = fname_escape(nvim, path).await?;
    let Some(path) = path.as_str() else {
        return Ok(());
    };

    nvim.exec_lua(
        "require('lazy-filer.call_lua').focus_on_last_active_win()",
        vec![],
    )
    .await?;
    new_edit(nvim, path).await?;

    Ok(())
}
