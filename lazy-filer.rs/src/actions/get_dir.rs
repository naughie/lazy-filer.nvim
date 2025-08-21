use super::NvimErr;

use nvim_router::nvim_rs::Value;

use super::renderer::LineIdx;
use super::{Action, States};

use std::path::Path;

pub struct GetDir {
    pub line_idx: LineIdx,
}

impl Action for GetDir {
    type Resp = Value;

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let Some(path) = states
            .actions
            .rendered_lines
            .get(self.line_idx)
            .and_then(|item| {
                if item.metadata.is_dir() {
                    path_to_val(&item.path)
                } else {
                    item.path.parent().and_then(path_to_val)
                }
            })
            .await
        else {
            return Ok(Value::Nil);
        };

        Ok(path)
    }
}

fn path_to_val(path: &Path) -> Option<Value> {
    path.to_str().map(Value::from)
}
