mod states;
use states::States;

mod fs;

mod actions;
use actions::prelude::*;

use std::error::Error;

use tokio::fs::File as TokioFile;

use nvim_rs::Buffer;
use nvim_rs::{Handler, Neovim, Value};
use nvim_rs::{compat::tokio::Compat, create::tokio as create};

type NvimWtr = <NeovimHandler as Handler>::Writer;
type NvimErr = Box<nvim_rs::error::CallError>;

#[derive(Clone)]
pub struct NeovimHandler {
    states: States,
}

impl NeovimHandler {
    async fn get_dir(&self, arg: &GetDir) -> Result<Value, Value> {
        arg.run(&self.states).await.map_err(|_| Value::Nil)
    }

    async fn create_entry(&self, arg: &CreateEntry) {
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
}

impl Handler for NeovimHandler {
    type Writer = Compat<TokioFile>;

    async fn handle_request(
        &self,
        name: String,
        args: Vec<Value>,
        _neovim: Neovim<Self::Writer>,
    ) -> Result<Value, Value> {
        if name == "get_dir" {
            let mut args = args.into_iter();

            let Some(line_idx) = args.next() else {
                return Ok(Value::Nil);
            };
            let Value::Integer(line_idx) = line_idx else {
                return Ok(Value::Nil);
            };
            let Some(line_idx) = line_idx.as_i64() else {
                return Ok(Value::Nil);
            };

            let arg = GetDir { line_idx };

            self.get_dir(&arg).await
        } else {
            Ok(Value::Nil)
        }
    }

    async fn handle_notify(&self, name: String, args: Vec<Value>, neovim: Neovim<Self::Writer>) {
        match name.as_str() {
            "create_entry" => {
                let mut args = args.into_iter();

                let Some(buf_id) = args.next() else {
                    return;
                };
                let Some(line_idx) = args.next() else {
                    return;
                };
                let Value::Integer(line_idx) = line_idx else {
                    return;
                };
                let Some(line_idx) = line_idx.as_i64() else {
                    return;
                };
                let Some(fname) = args.next() else {
                    return;
                };
                let Value::String(fname) = fname else {
                    return;
                };
                let Some(fname) = fname.into_str() else {
                    return;
                };

                let buf = Buffer::new(buf_id, neovim.clone());

                let arg = CreateEntry {
                    buf,
                    line_idx,
                    fname,
                };

                self.create_entry(&arg).await;
            }
            "new_filer" => {
                let mut args = args.into_iter();

                let Some(buf_id) = args.next() else {
                    return;
                };
                let Some(dir) = args.next() else {
                    return;
                };
                let Value::String(dir) = dir else {
                    return;
                };
                let Some(dir) = dir.into_str() else {
                    return;
                };

                let buf = Buffer::new(buf_id, neovim.clone());

                let arg = NewFiler {
                    buf,
                    dir: dir.into(),
                    nvim: neovim,
                };

                self.new_filer(&arg).await;
            }
            "move_to_parent" => {
                let mut args = args.into_iter();

                let Some(buf_id) = args.next() else {
                    return;
                };
                let Some(dir) = args.next() else {
                    return;
                };
                let Value::String(dir) = dir else {
                    return;
                };
                let Some(dir) = dir.into_str() else {
                    return;
                };

                let buf = Buffer::new(buf_id, neovim);

                let arg = MoveToParent {
                    buf,
                    dir: dir.into(),
                };

                self.move_to_parent(&arg).await;
            }
            "open_file" => {
                let mut args = args.into_iter();

                let Some(line_idx) = args.next() else {
                    return;
                };
                let Value::Integer(line_idx) = line_idx else {
                    return;
                };
                let Some(line_idx) = line_idx.as_i64() else {
                    return;
                };

                let arg = OpenFile {
                    line_idx,
                    nvim: neovim,
                };

                self.open_file(&arg).await;
            }
            "expand_dir" => {
                let mut args = args.into_iter();

                let Some(buf_id) = args.next() else {
                    return;
                };
                let Some(line_idx) = args.next() else {
                    return;
                };
                let Value::Integer(line_idx) = line_idx else {
                    return;
                };
                let Some(line_idx) = line_idx.as_i64() else {
                    return;
                };

                let buf = Buffer::new(buf_id, neovim.clone());

                let arg = ExpandDir { line_idx, buf };

                self.expand_dir(&arg).await;
            }
            "open_or_expand" => {
                let mut args = args.into_iter();

                let Some(buf_id) = args.next() else {
                    return;
                };
                let Some(line_idx) = args.next() else {
                    return;
                };
                let Value::Integer(line_idx) = line_idx else {
                    return;
                };
                let Some(line_idx) = line_idx.as_i64() else {
                    return;
                };

                let buf = Buffer::new(buf_id, neovim.clone());

                let arg = OpenOrExpand {
                    line_idx,
                    buf,
                    nvim: neovim,
                };

                self.open_or_expand(&arg).await;
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let handler: NeovimHandler = NeovimHandler {
        states: Default::default(),
    };

    let (nvim, io_handler) = create::new_parent(handler).await?;

    // Any error should probably be logged, as stderr is not visible to users.
    match io_handler.await {
        Err(joinerr) => eprintln!("Error joining IO loop: '{joinerr}'"),
        Ok(Err(err)) => {
            if !err.is_reader_error() {
                // One last try, since there wasn't an error with writing to the
                // stream
                nvim.err_writeln(&format!("Error: '{err}'"))
                    .await
                    .unwrap_or_else(|e| {
                        // We could inspect this error to see what was happening, and
                        // maybe retry, but at this point it's probably best
                        // to assume the worst and print a friendly and
                        // supportive message to our users
                        eprintln!("Well, dang... '{e}'");
                    });
            }

            if !err.is_channel_closed() {
                // Closed channel usually means neovim quit itself, or this plugin was
                // told to quit by closing the channel, so it's not always an error
                // condition.
                eprintln!("Error: '{err}'");

                let mut source = err.source();

                while let Some(e) = source {
                    eprintln!("Caused by: '{e}'");
                    source = e.source();
                }
            }
        }
        Ok(Ok(())) => {}
    }

    Ok(())
}
