use crate::tools::common::{ensure_args_at_least, run_process};
use domain::tools::{Tool, ToolError, ToolOutput};

pub struct ApplyPatchTool;

impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }

    fn description(&self) -> &str {
        "Apply a unified diff patch file"
    }

    fn usage(&self) -> &str {
        "apply_patch <patch_file>"
    }

    fn examples(&self) -> Vec<&str> {
        vec!["apply_patch /tmp/fix.diff"]
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    fn execute(&self, args: &[&str]) -> Result<ToolOutput, ToolError> {
        ensure_args_at_least(args, 1, self.usage())?;
        run_process("patch", &["-p0", "-i", args[0]])
    }
}
