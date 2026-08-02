use super::*;

pub(super) type ReturnLowerer =
    fn(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>;

pub(in crate::ir::lower) struct LoweredNonterminalBlock {
    pub(in crate::ir::lower) instructions: Vec<Instruction>,
    pub(in crate::ir::lower) ends_execution: bool,
}

pub(in crate::ir::lower) enum TerminalBranch<'a> {
    Statement(&'a Stmt),
    Result(&'a Expr),
}
