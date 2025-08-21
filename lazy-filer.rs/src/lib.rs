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
    async fn get_dir(&self, arg: &GetDir) -> Result<Value, Value> {
        arg.run(&self.states).await.map_err(|_| Value::Nil)
    }

    async fn get_file_path(&self, arg: &GetFilePath) -> Result<Value, Value> {
        arg.run(&self.states).await.map_err(|_| Value::Nil)
    }

    async fn create_entry(&self, arg: &CreateEntry) {
        arg.run(&self.states).await.ok();
    }

    async fn delete_entry(&self, arg: &DeleteEntry) {
        arg.run(&self.states).await.ok();
    }

    async fn expand_dir(&self, arg: &ExpandDir) {
        arg.run(&self.states).await.ok();
    }

    async fn move_to_parent(&self, arg: &MoveToParent) {
        arg.run(&self.states).await.ok();
    }

    async fn new_filer(&self, arg: &NewFiler) {
        arg.run(&self.states).await.ok();
    }

    async fn open_file(&self, arg: &OpenFile) {
        arg.run(&self.states).await.ok();
    }

    async fn open_or_expand(&self, arg: &OpenOrExpand) {
        arg.run(&self.states).await.ok();
    }

    async fn refresh(&self, arg: &Refresh) {
        arg.run(&self.states).await.ok();
    }

    async fn rename_entry(&self, arg: &RenameEntry) {
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

                self.get_dir(&arg).await
            }
            "get_file_path" => {
                let Some(line_idx) = args.next_i64() else {
                    return Ok(Value::Nil);
                };
                let line_idx = line_idx.into();

                let arg = GetFilePath { line_idx };

                self.get_file_path(&arg).await
            }
            _ => Ok(Value::Nil),
        }
    }

    async fn handle_notify(&self, name: &str, mut args: RpcArgs, nvim: Neovim<NvimWtr>) {
        match name {
            "create_entry" => {
                let Some(buf) = args.next_buf(&nvim) else {
                    return;
                };
                let Some(line_idx) = args.next_i64() else {
                    return;
                };
                let line_idx = line_idx.into();
                let Some(fname) = args.next_string() else {
                    return;
                };

                let arg = CreateEntry {
                    nvim,
                    buf,
                    line_idx,
                    fname,
                };

                self.create_entry(&arg).await;
            }
            "delete_entry" => {
                let Some(buf) = args.next_buf(&nvim) else {
                    return;
                };
                let Some(line_idx) = args.next_i64() else {
                    return;
                };
                let line_idx = line_idx.into();

                let arg = DeleteEntry {
                    nvim,
                    buf,
                    line_idx,
                };

                self.delete_entry(&arg).await;
            }
            "rename_entry" => {
                let Some(buf) = args.next_buf(&nvim) else {
                    return;
                };
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
                    buf,
                    line_idx,
                    dir: dir.into(),
                    path,
                };

                self.rename_entry(&arg).await;
            }
            "new_filer" => {
                let Some(buf) = args.next_buf(&nvim) else {
                    return;
                };
                let Some(dir) = args.next_string() else {
                    return;
                };

                let arg = NewFiler {
                    nvim,
                    buf,
                    dir: dir.into(),
                };

                self.new_filer(&arg).await;
            }
            "refresh" => {
                let Some(buf) = args.next_buf(&nvim) else {
                    return;
                };
                let Some(dir) = args.next_string() else {
                    return;
                };

                let arg = Refresh {
                    nvim,
                    buf,
                    dir: dir.into(),
                };

                self.refresh(&arg).await;
            }
            "move_to_parent" => {
                let Some(buf) = args.next_buf(&nvim) else {
                    return;
                };
                let Some(dir) = args.next_string() else {
                    return;
                };

                let arg = MoveToParent {
                    nvim,
                    buf,
                    dir: dir.into(),
                };

                self.move_to_parent(&arg).await;
            }
            "open_file" => {
                let Some(line_idx) = args.next_i64() else {
                    return;
                };
                let line_idx = line_idx.into();

                let arg = OpenFile { line_idx, nvim };

                self.open_file(&arg).await;
            }
            "expand_dir" => {
                let Some(buf) = args.next_buf(&nvim) else {
                    return;
                };
                let Some(line_idx) = args.next_i64() else {
                    return;
                };
                let line_idx = line_idx.into();

                let arg = ExpandDir {
                    line_idx,
                    nvim,
                    buf,
                };

                self.expand_dir(&arg).await;
            }
            "open_or_expand" => {
                let Some(buf) = args.next_buf(&nvim) else {
                    return;
                };
                let Some(line_idx) = args.next_i64() else {
                    return;
                };
                let line_idx = line_idx.into();

                let arg = OpenOrExpand {
                    line_idx,
                    buf,
                    nvim,
                };

                self.open_or_expand(&arg).await;
            }
            _ => {}
        }
    }
}
