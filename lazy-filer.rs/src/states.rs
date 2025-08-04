use crate::actions::states::States as ActionStates;
use crate::fs::RootFile;

#[derive(Debug, Clone, Default)]
pub struct States {
    pub root_file: RootFile,
    pub actions: ActionStates,
}
