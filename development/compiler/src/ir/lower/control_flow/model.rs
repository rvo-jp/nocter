use super::*;

pub(super) type ReturnLowerer =
    fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>;

pub(super) struct LoweredNonterminalBlock {
    pub(super) instructions: Vec<Instruction>,
    pub(super) ends_execution: bool,
}

pub(in crate::ir::lower) enum TerminalBranch<'a> {
    Statement(&'a Stmt),
    Result(&'a Expr),
}
