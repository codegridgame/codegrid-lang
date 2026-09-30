use crate::{
    evaluate::{EvaluationConfig, EvaluationMode},
    metrics::Metrics,
    validate::ProgramRejection,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvaluationStatus {
    Passed,
    ProgramRejected(ProgramRejection),
    TestFailed,
    RuntimeError,
    ConstraintExceeded,
    ResourceLimitExceeded,
    Cancelled,
    Fault(FaultReason),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FaultReason {
    NumericOverflow,
    VmFault,
    VmInitialization,
}
impl FaultReason {
    /// Stable fault identity assigned by the evaluator at detection.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NumericOverflow => "level.metric_overflow",
            Self::VmFault => "level.vm_fault",
            Self::VmInitialization => "level.vm_initialization_fault",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TestOutcome {
    Passed,
    WrongOutput,
    IncompleteOutput,
    RuntimeError(Vec<String>),
}
/// Only visible test details can inhabit public result types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibleTestResult {
    pub source_index: usize,
    pub outcome: TestOutcome,
    pub input: Vec<u8>,
    pub expected_output: Vec<u8>,
    pub actual_output: Vec<u8>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvaluationResult {
    pub level_id: String,
    pub level_version: u32,
    pub evaluator_contract: String,
    pub mode: EvaluationMode,
    pub config: EvaluationConfig,
    pub vm_seed: u64,
    pub status: EvaluationStatus,
    pub hidden_failure: Option<TestOutcome>,
    pub visible_tests: Vec<VisibleTestResult>,
    pub partial_metrics: Metrics,
    pub final_metrics: Option<Metrics>,
    pub rating: Option<u8>,
    pub constraints: Vec<ConstraintResult>,
    pub scoring: Vec<ScoringResult>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstraintResult {
    pub name: String,
    pub limit: u64,
    pub value: u64,
    pub passed: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoringResult {
    pub name: String,
    pub target: Option<u64>,
    pub value: u64,
    pub rating: Option<u8>,
}
