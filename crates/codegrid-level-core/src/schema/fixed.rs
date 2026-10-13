use super::{integer, object, BoardBounds, LevelError};
use codegrid_ir::Cell;
use codegrid_model::{AttachmentInstruction, ConditionPrefix, PrimaryInstruction};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixedCell {
    pub x: u32,
    pub y: u32,
    pub instruction: Cell,
}

pub(super) fn parse_fixed_cells(
    value: &Value,
    bounds: &BoardBounds,
) -> Result<Vec<FixedCell>, LevelError> {
    let path = "$.program_rules.fixed_cells";
    let cells = value
        .as_array()
        .ok_or_else(|| LevelError::invalid("IncorrectType", path))?;
    if cells.len() > bounds.width as usize * bounds.height as usize {
        return Err(LevelError::invalid("TooManyFixedCells", path));
    }
    let mut seen = BTreeSet::new();
    cells
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let p = format!("{path}[{index}]");
            let cell = object(value, &p, &["x", "y", "instruction"])?;
            let x = integer(
                &cell["x"],
                &format!("{p}.x"),
                u64::from(bounds.width - 1),
                false,
            )? as u32;
            let y = integer(
                &cell["y"],
                &format!("{p}.y"),
                u64::from(bounds.height - 1),
                false,
            )? as u32;
            if !seen.insert((x, y)) {
                return Err(LevelError::invalid("DuplicateFixedCell", &p));
            }
            let ip = format!("{p}.instruction");
            let i = object(&cell["instruction"], &ip, &["main", "prefix", "suffix"])?;
            let primary = i["main"]
                .as_str()
                .and_then(PrimaryInstruction::from_token)
                .ok_or_else(|| {
                    LevelError::invalid("InvalidFixedInstruction", &format!("{ip}.main"))
                })?;
            let prefix = match &i["prefix"] {
                Value::Null => None,
                Value::String(s) => Some(match s.as_str() {
                    "?0" => ConditionPrefix::Zero,
                    "?1" => ConditionPrefix::One,
                    "?2" => ConditionPrefix::Two,
                    "?!" => ConditionPrefix::Flag,
                    _ => {
                        return Err(LevelError::invalid(
                            "InvalidFixedInstruction",
                            &format!("{ip}.prefix"),
                        ))
                    }
                }),
                _ => {
                    return Err(LevelError::invalid(
                        "IncorrectType",
                        &format!("{ip}.prefix"),
                    ))
                }
            };
            let attachment = match &i["suffix"] {
                Value::Null => None,
                Value::String(s) => {
                    Some(AttachmentInstruction::from_token(s).ok_or_else(|| {
                        LevelError::invalid("InvalidFixedInstruction", &format!("{ip}.suffix"))
                    })?)
                }
                _ => {
                    return Err(LevelError::invalid(
                        "IncorrectType",
                        &format!("{ip}.suffix"),
                    ))
                }
            };
            if let Some(suffix) = attachment {
                let immediate_repeat = matches!(primary, PrimaryInstruction::OutputImmediate(_))
                    && matches!(suffix, AttachmentInstruction::Repeat(_));
                if (!primary.is_encodable() && !immediate_repeat)
                    || (matches!(suffix, AttachmentInstruction::Repeat(_))
                        && matches!(
                            primary,
                            PrimaryInstruction::Call(_) | PrimaryInstruction::Return
                        ))
                {
                    return Err(LevelError::invalid(
                        "InvalidFixedInstruction",
                        &format!("{ip}.suffix"),
                    ));
                }
            }
            Ok(FixedCell {
                x,
                y,
                instruction: Cell {
                    primary: Some(primary),
                    prefix,
                    attachment,
                    entry: None,
                },
            })
        })
        .collect()
}
