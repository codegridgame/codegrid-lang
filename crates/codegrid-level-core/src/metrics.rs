use crate::validate::{attachment_kind, condition_kind, instruction_kind};
use codegrid_ir::{ScopedProgram, VerifiedProgram};
use codegrid_vm::RuntimeMetricSummary;
use std::collections::{BTreeMap, BTreeSet};

pub type Metrics = BTreeMap<String, u64>;

pub fn static_metrics(program: &VerifiedProgram) -> Metrics {
    let mut cells = 0;
    let mut boards = 0;
    let mut functions = 0;
    let mut kinds = BTreeSet::new();
    for scoped in std::iter::once(&program.program().outer)
        .chain(program.program().customs.values().map(|c| &c.program))
    {
        measure_scope(scoped, &mut cells, &mut boards, &mut functions, &mut kinds);
    }
    BTreeMap::from([
        ("non_empty_cells".into(), cells),
        ("instruction_kinds".into(), kinds.len() as u64),
        ("functions_used".into(), functions),
        ("boards_used".into(), boards),
    ])
}
fn measure_scope(
    scoped: &ScopedProgram,
    cells: &mut u64,
    boards: &mut u64,
    functions: &mut u64,
    kinds: &mut BTreeSet<&'static str>,
) {
    *functions += scoped.functions.len() as u64;
    for board in std::iter::once(&scoped.main).chain(scoped.functions.values()) {
        *boards += 1 + board.folded_blocks.len() as u64;
        for cell in &board.cells {
            if cell.entry.is_some() || cell.primary.is_some() {
                *cells += 1;
            }
            if let Some(p) = cell.primary {
                kinds.insert(instruction_kind(p));
            }
            if let Some(prefix) = cell.prefix {
                kinds.insert(condition_kind(prefix));
            }
            if let Some(a) = cell.attachment {
                kinds.insert(attachment_kind(a));
            }
        }
        for folded in board.folded_blocks.values() {
            for prefix in folded.prefixes.values() {
                kinds.insert(condition_kind(*prefix));
            }
            for p in folded.cells.iter().flatten() {
                *cells += 1;
                kinds.insert(instruction_kind(*p));
            }
        }
    }
}
pub fn dynamic_metrics(raw: &RuntimeMetricSummary) -> Metrics {
    BTreeMap::from([
        ("ticks".into(), raw.global_tick()),
        ("cost".into(), raw.operation_count()),
        ("operation_count".into(), raw.operation_count()),
        (
            "memory_addresses_used".into(),
            raw.used_memory_address_count() as u64,
        ),
        ("max_data_stack_depth".into(), raw.peak_data_stack_usage()),
        (
            "max_instruction_stack_depth".into(),
            raw.peak_instruction_stack_usage(),
        ),
        ("max_call_stack_depth".into(), raw.peak_call_stack_usage()),
    ])
}
pub fn aggregate(base: &Metrics, current: &Metrics) -> Option<Metrics> {
    let mut result = base.clone();
    for (name, value) in current {
        let old = result.get(name).copied().unwrap_or(0);
        let next = if matches!(
            name.as_str(),
            "ticks" | "cost" | "operation_count" | "travel_distance" | "stop_count"
        ) {
            old.checked_add(*value)?
        } else {
            old.max(*value)
        };
        result.insert(name.clone(), next);
    }
    Some(result)
}
pub fn constraints_exceeded(metrics: &Metrics, constraints: &BTreeMap<String, u64>) -> bool {
    constraints.iter().any(|(key, limit)| {
        let metric = constraint_metric(key);
        metrics.get(metric).is_some_and(|v| v > limit)
    })
}
pub fn constraint_metric(key: &str) -> &'static str {
    match key {
        "max_ticks" => "ticks",
        "max_cost" => "cost",
        "max_operation_count" => "operation_count",
        "max_memory_addresses" => "memory_addresses_used",
        "max_data_stack_depth" => "max_data_stack_depth",
        "max_instruction_stack_depth" => "max_instruction_stack_depth",
        "max_call_stack_depth" => "max_call_stack_depth",
        "max_non_empty_cells" => "non_empty_cells",
        "max_instruction_kinds" => "instruction_kinds",
        "max_functions_used" => "functions_used",
        "max_boards_used" => "boards_used",
        "max_travel_distance" => "travel_distance",
        "max_stop_count" => "stop_count",
        _ => "",
    }
}
pub fn rating(metrics: &Metrics, scoring: &BTreeMap<String, Option<u64>>) -> Option<u8> {
    scoring
        .iter()
        .filter_map(|(name, target)| {
            target.map(|target| {
                let value = metrics.get(name).copied().unwrap_or(0);
                if value <= target {
                    3
                } else if u128::from(value) <= (u128::from(target) * 3).div_ceil(2) {
                    2
                } else {
                    1
                }
            })
        })
        .min()
}
