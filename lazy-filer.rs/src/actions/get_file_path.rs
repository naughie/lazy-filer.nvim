use super::NvimErr;

use nvim_rs::Value;

use super::renderer::LineIdx;
use super::{Action, States};

use std::path::Path;

pub struct GetFilePath {
    pub line_idx: LineIdx,
}

impl Action for GetFilePath {
    type Resp = Value;

    async fn run(&self, states: &States) -> Result<Self::Resp, NvimErr> {
        let Some(path) = states
            .actions
            .rendered_lines
            .get(self.line_idx)
            .and_then(|item| path_to_val(&item.path))
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
