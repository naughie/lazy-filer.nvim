mod states;
use states::States;

mod fs;

mod actions;
use actions::prelude::*;

use tokio::fs::File as TokioFile;

use nvim_router::RpcArgs;
use nvim_router::nvim_rs::compat::tokio::Compat;
use nvim_router::nvim_rs::{Neovim, Value};

type NvimWtr = Compat<TokioFile>;
type NvimErr = Box<nvim_router::nvim_rs::error::CallError>;

#[derive(Clone)]
pub struct NeovimHandler {
    states: States,
}

impl NeovimHandler {
    async fn request(&self, arg: &impl Action<Resp = Value>) -> Result<Value, Value> {
        arg.run(&self.states).await.map_err(|_| Value::Nil)
    }

    async fn notify(&self, arg: &impl Action<Resp = ()>) {
        arg.run(&self.states).await.ok();
    }
}

impl nvim_router::NeovimHandler<NvimWtr> for NeovimHandler {
    fn new() -> Self {
        Self {
            states: Default::default(),
        }
    }

    async fn handle_request(
        &self,
        name: &str,
        mut args: RpcArgs,
        _neovim: Neovim<NvimWtr>,
    ) -> Result<Value, Value> {
        match name {
            "get_dir" => {
                let Some(line_idx) = args.next_i64() else {
                    return Ok(Value::Nil);
                };
                let line_idx = line_idx.into();

                let arg = GetDir { line_idx };

                self.request(&arg).await
            }
            "get_file_path" => {
                let Some(line_idx) = args.next_i64() else {
                    return Ok(Value::Nil);
                };
                let line_idx = line_idx.into();

                let arg = GetFilePath { line_idx };

                self.request(&arg).await
            }
            _ => Ok(Value::Nil),
        }
    }

    async fn handle_notify(&self, name: &str, mut args: RpcArgs, nvim: Neovim<NvimWtr>) {
        match name {
            "create_entry" => {
                let Some(line_idx) = args.next_i64() else {
                    return;
                };
                let line_idx = line_idx.into();
                let Some(fname) = args.next_string() else {
                    return;
                };

                let arg = CreateEntry {
                    nvim,
                    line_idx,
                    fname,
                };

                self.notify(&arg).await;
            }
            "delete_entry" => {
                let Some(line_idx) = args.next_i64() else {
                    return;
                };
                let line_idx = line_idx.into();

                let arg = DeleteEntry { nvim, line_idx };

                self.notify(&arg).await;
            }
            "rename_entry" => {
                let Some(line_idx) = args.next_i64() else {
                    return;
                };
                let line_idx = line_idx.into();
                let Some(dir) = args.next_string() else {
                    return;
                };
                let Some(path) = args.next_string() else {
                    return;
                };

                let arg = RenameEntry {
                    nvim,
                    line_idx,
                    dir: dir.into(),
                    path,
                };

                self.notify(&arg).await;
            }
            "new_filer" => {
                let Some(dir) = args.next_string() else {
                    return;
                };

                let arg = NewFiler {
                    nvim,
                    dir: dir.into(),
                };

                self.notify(&arg).await;
            }
            "refresh" => {
                let Some(dir) = args.next_string() else {
                    return;
                };

                let arg = Refresh {
                    nvim,
                    dir: dir.into(),
                };

                self.notify(&arg).await;
            }
            "move_to_parent" => {
                let Some(dir) = args.next_string() else {
                    return;
                };

                let arg = MoveToParent {
                    nvim,
                    dir: dir.into(),
                };

                self.notify(&arg).await;
            }
            "open_file" => {
                let Some(line_idx) = args.next_i64() else {
                    return;
                };
                let line_idx = line_idx.into();

                let arg = OpenFile { line_idx, nvim };

                self.notify(&arg).await;
            }
            "expand_dir" => {
                let Some(line_idx) = args.next_i64() else {
                    return;
                };
                let line_idx = line_idx.into();

                let arg = ExpandDir { line_idx, nvim };

                self.notify(&arg).await;
            }
            "open_or_expand" => {
                let Some(line_idx) = args.next_i64() else {
                    return;
                };
                let line_idx = line_idx.into();

                let arg = OpenOrExpand { line_idx, nvim };

                self.notify(&arg).await;
            }
            _ => {}
        }
    }
}
