use super::{integer, LevelError};
use crate::validate::{
    attachment_identifiers, attachment_kind, condition_kind, instruction_identifiers,
    instruction_kind, ProgramRejection,
};
use codegrid_ir::{Board, VerifiedProgram};
use codegrid_model::PrimaryInstruction;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn parse(value: &Value) -> Result<BTreeMap<String, u32>, LevelError> {
    let path = "$.program_rules.instruction_limits";
    let map = value
        .as_object()
        .ok_or_else(|| LevelError::invalid("IncorrectType", path))?;
    if map.len() > 64 {
        return Err(LevelError::invalid("TooManyInstructionLimits", path));
    }
    map.iter()
        .map(|(key, value)| {
            let path = format!("{path}.{key}");
            if !instruction_identifiers().contains(&key.as_str())
                && !attachment_identifiers().contains(&key.as_str())
                && members(key).is_none()
            {
                return Err(LevelError::invalid("UnsupportedInstructionLimit", &path));
            }
            Ok((
                key.clone(),
                integer(value, &path, u64::from(u32::MAX), false)? as u32,
            ))
        })
        .collect()
}
fn members(key: &str) -> Option<&'static [&'static str]> {
    Some(match key {
        "DIRECTION" => &["MOVE_UP", "MOVE_DOWN", "MOVE_LEFT", "MOVE_RIGHT"],
        "REGISTER_POINTER" => &["POINTER_LEFT", "POINTER_RIGHT"],
        "STACK" => &["PUSH", "POP_ADD"],
        "CODEC" => &["DECODE", "ENCODE"],
        "MEMORY" => &[
            "MEMORY_LOAD",
            "MEMORY_STORE",
            "PAGE_INCREMENT",
            "PAGE_DECREMENT",
        ],
        "PAGE" => &["PAGE_INCREMENT", "PAGE_DECREMENT"],
        "SHIFT" => &["SHIFT_LEFT", "SHIFT_RIGHT"],
        "CALL" => &["CALL", "RETURN"],
        "CUSTOM" => &["CUSTOM", "CUSTOM_RETURN"],
        "UNARY" => &["CLEAR", "ADD", "SUB", "NEG", "SHIFT_LEFT", "SHIFT_RIGHT"],
        "BINARY" => &["NAND", "CMP"],
        "SELF_MODIFY" => &["DECODE", "ENCODE", "READ_CODE", "WRITE_CODE"],
        "PREFIX_CONDITION" => &[
            "CONDITION_0",
            "CONDITION_1",
            "CONDITION_2",
            "CONDITION_FLAG",
        ],
        "CUSTOMIZATION" => &["CALL", "RETURN", "CUSTOM", "CUSTOM_RETURN", "FOLDED_BLOCK"],
        "SPECIAL" => &["RANDOM_DIRECTION", "REPEAT", "OUTPUT_IMMEDIATE"],
        _ => return None,
    })
}
pub(crate) fn validate(
    limits: &BTreeMap<String, u32>,
    program: &VerifiedProgram,
) -> Result<(), ProgramRejection> {
    if limits.is_empty() {
        return Ok(());
    }
    let mut counts: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut add = |kind: &'static str| {
        *counts.entry(kind).or_default() += 1;
    };
    let primary = |p| {
        if matches!(p, PrimaryInstruction::OutputImmediate(_)) {
            "OUTPUT_IMMEDIATE"
        } else {
            instruction_kind(p)
        }
    };
    let mut board = |b: &Board| {
        for c in &b.cells {
            if let Some(p) = c.primary {
                add(primary(p));
            }
            if let Some(p) = c.prefix {
                add(condition_kind(p));
            }
            if let Some(a) = c.attachment {
                add(attachment_kind(a));
            }
        }
        for folded in b.folded_blocks.values() {
            for p in folded.cells.iter().flatten() {
                add(primary(*p));
            }
            for p in folded.prefixes.values() {
                add(condition_kind(*p));
            }
        }
    };
    for scope in std::iter::once(&program.program().outer)
        .chain(program.program().customs.values().map(|c| &c.program))
    {
        board(&scope.main);
        for b in scope.functions.values() {
            board(b);
        }
    }
    for (key, max) in limits {
        let used = match members(key) {
            Some(kinds) => kinds
                .iter()
                .map(|kind| counts.get(kind).copied().unwrap_or(0))
                .sum(),
            None => counts.get(key.as_str()).copied().unwrap_or(0),
        };
        if used > u64::from(*max) {
            return Err(ProgramRejection {
                reason: "InstructionCountExceeded",
                path: format!("$.program_rules.instruction_limits.{key}"),
            });
        }
    }
    Ok(())
}
