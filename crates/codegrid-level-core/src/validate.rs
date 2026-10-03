use crate::{ProgramRules, ValidatedLevel};
use codegrid_ir::{Board, ScopedProgram, VerifiedProgram};
use codegrid_model::{
    AttachmentInstruction, ConditionPrefix, Direction, PageDirection, PointerDirection,
    PrimaryInstruction, ShiftDirection,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRejection {
    pub reason: &'static str,
    pub path: String,
}
fn reject(reason: &'static str, path: &str) -> ProgramRejection {
    ProgramRejection {
        reason,
        path: path.into(),
    }
}
pub fn attachment_identifiers() -> &'static [&'static str] {
    &[
        "READ_CODE",
        "WRITE_CODE",
        "REPEAT",
        "CONDITION_0",
        "CONDITION_1",
        "CONDITION_2",
    ]
}
pub fn instruction_identifiers() -> &'static [&'static str] {
    &[
        "REGISTER_POINTER",
        "STACK",
        "CODEC",
        "MEMORY",
        "PAGE",
        "SHIFT",
        "MOVE_UP",
        "MOVE_DOWN",
        "MOVE_LEFT",
        "MOVE_RIGHT",
        "RANDOM_DIRECTION",
        "CMP",
        "READ",
        "READ_UP",
        "READ_DOWN",
        "READ_LEFT",
        "READ_RIGHT",
        "CLEAR",
        "ADD",
        "SUB",
        "POINTER_LEFT",
        "POINTER_RIGHT",
        "OUTPUT",
        "PUSH",
        "POP_ADD",
        "DECODE",
        "ENCODE",
        "CALL",
        "RETURN",
        "NAND",
        "MEMORY_LOAD",
        "MEMORY_STORE",
        "PAGE_INCREMENT",
        "PAGE_DECREMENT",
        "SHIFT_LEFT",
        "SHIFT_RIGHT",
        "FOLDED_BLOCK",
        "CUSTOM",
        "CUSTOM_RETURN",
        "HALT",
    ]
}
pub fn instruction_kind(p: PrimaryInstruction) -> &'static str {
    match p {
        PrimaryInstruction::Direction(Direction::Up) => "MOVE_UP",
        PrimaryInstruction::Direction(Direction::Down) => "MOVE_DOWN",
        PrimaryInstruction::Direction(Direction::Left) => "MOVE_LEFT",
        PrimaryInstruction::Direction(Direction::Right) => "MOVE_RIGHT",
        PrimaryInstruction::RandomDirection => "RANDOM_DIRECTION",
        PrimaryInstruction::Compare => "CMP",
        PrimaryInstruction::Read(_) => "READ",
        PrimaryInstruction::Clear => "CLEAR",
        PrimaryInstruction::Add => "ADD",
        PrimaryInstruction::Sub => "SUB",
        PrimaryInstruction::MoveRegisterPointer(PointerDirection::Left) => "POINTER_LEFT",
        PrimaryInstruction::MoveRegisterPointer(PointerDirection::Right) => "POINTER_RIGHT",
        PrimaryInstruction::Output | PrimaryInstruction::OutputImmediate(_) => "OUTPUT",
        PrimaryInstruction::Push => "PUSH",
        PrimaryInstruction::PopAdd => "POP_ADD",
        PrimaryInstruction::Decode => "DECODE",
        PrimaryInstruction::Encode => "ENCODE",
        PrimaryInstruction::Call(_) => "CALL",
        PrimaryInstruction::Return => "RETURN",
        PrimaryInstruction::Nand => "NAND",
        PrimaryInstruction::MemoryLoad => "MEMORY_LOAD",
        PrimaryInstruction::MemoryStore => "MEMORY_STORE",
        PrimaryInstruction::MovePage(PageDirection::Increment) => "PAGE_INCREMENT",
        PrimaryInstruction::MovePage(PageDirection::Decrement) => "PAGE_DECREMENT",
        PrimaryInstruction::Shift(ShiftDirection::Left) => "SHIFT_LEFT",
        PrimaryInstruction::Shift(ShiftDirection::Right) => "SHIFT_RIGHT",
        PrimaryInstruction::FoldedBlock(_) => "FOLDED_BLOCK",
        PrimaryInstruction::Custom(_) => "CUSTOM",
        PrimaryInstruction::CustomReturn => "CUSTOM_RETURN",
        PrimaryInstruction::Halt => "HALT",
    }
}
pub fn condition_kind(prefix: ConditionPrefix) -> &'static str {
    match prefix {
        ConditionPrefix::Zero => "CONDITION_0",
        ConditionPrefix::One => "CONDITION_1",
        ConditionPrefix::Two => "CONDITION_2",
    }
}
pub fn attachment_kind(a: AttachmentInstruction) -> &'static str {
    match a {
        AttachmentInstruction::ReadCode => "READ_CODE",
        AttachmentInstruction::WriteCode => "WRITE_CODE",
        AttachmentInstruction::Repeat(_) => "REPEAT",
    }
}
pub fn validate_primary(
    r: &ProgramRules,
    p: Option<PrimaryInstruction>,
    generated: bool,
) -> Result<(), ProgramRejection> {
    let Some(p) = p else { return Ok(()) };
    if !r.memory_enabled
        && matches!(
            p,
            PrimaryInstruction::MemoryLoad
                | PrimaryInstruction::MemoryStore
                | PrimaryInstruction::MovePage(_)
        )
    {
        return Err(reject(
            if generated {
                "GeneratedMemoryNotAllowed"
            } else {
                "MemoryNotAllowed"
            },
            "",
        ));
    }
    let directional = match p {
        PrimaryInstruction::Read(Direction::Up) => Some("READ_UP"),
        PrimaryInstruction::Read(Direction::Down) => Some("READ_DOWN"),
        PrimaryInstruction::Read(Direction::Left) => Some("READ_LEFT"),
        PrimaryInstruction::Read(Direction::Right) => Some("READ_RIGHT"),
        _ => None,
    };
    let default_allowed = matches!(
        p,
        PrimaryInstruction::Direction(_)
            | PrimaryInstruction::Output
            | PrimaryInstruction::OutputImmediate(_)
            | PrimaryInstruction::Halt
    );
    let group = match p {
        PrimaryInstruction::Read(_) => Some("READ"),
        PrimaryInstruction::MoveRegisterPointer(_) => Some("REGISTER_POINTER"),
        PrimaryInstruction::Push | PrimaryInstruction::PopAdd => Some("STACK"),
        PrimaryInstruction::Decode | PrimaryInstruction::Encode => Some("CODEC"),
        PrimaryInstruction::MemoryLoad | PrimaryInstruction::MemoryStore => Some("MEMORY"),
        PrimaryInstruction::MovePage(_) => Some("PAGE"),
        PrimaryInstruction::Shift(_) => Some("SHIFT"),
        PrimaryInstruction::Return => Some("CALL"),
        PrimaryInstruction::CustomReturn => Some("CUSTOM"),
        _ => None,
    };
    if !default_allowed
        && !group.is_some_and(|name| r.allowed_instructions.contains(name))
        && !r.allowed_instructions.contains(instruction_kind(p))
        && !directional.is_some_and(|name| r.allowed_instructions.contains(name))
    {
        return Err(reject(
            if generated {
                "GeneratedInstructionNotAllowed"
            } else {
                "InstructionNotAllowed"
            },
            "",
        ));
    }
    Ok(())
}
pub fn validate_program(
    level: &ValidatedLevel,
    program: &VerifiedProgram,
) -> Result<(), ProgramRejection> {
    let r = level.rules();
    let p = program.program();
    if p.customs.len() > r.max_custom as usize {
        return Err(reject("CustomCountExceeded", "customs"));
    }
    scoped(r, &p.outer, "outer")?;
    for (slot, c) in &p.customs {
        scoped(r, &c.program, &format!("customs[{}]", slot.get()))?;
    }
    Ok(())
}
fn scoped(r: &ProgramRules, s: &ScopedProgram, path: &str) -> Result<(), ProgramRejection> {
    if s.functions.len() > r.max_functions as usize {
        return Err(reject("FunctionCountExceeded", path));
    }
    if s.main.cells.iter().filter(|c| c.entry.is_some()).count() > r.max_threads as usize {
        return Err(reject("ThreadCountExceeded", path));
    }
    board(r, &s.main, true, &format!("{path}.main"))?;
    for (slot, b) in &s.functions {
        board(r, b, false, &format!("{path}.functions[{}]", slot.get()))?;
    }
    Ok(())
}
fn board(r: &ProgramRules, b: &Board, main: bool, path: &str) -> Result<(), ProgramRejection> {
    let limit = if main {
        &r.main_board
    } else {
        &r.function_board
    };
    if b.width > limit.width as usize || b.height > limit.height as usize {
        return Err(reject("BoardDimensionsExceeded", path));
    }
    for (i, c) in b.cells.iter().enumerate() {
        let path = format!("{path}.cells[{i}]");
        validate_primary(r, c.primary, false).map_err(|mut e| {
            e.path = path.clone();
            e
        })?;
        if let Some(prefix) = c.prefix {
            if !r.allowed_attachments.contains(condition_kind(prefix)) {
                return Err(reject("AttachmentNotAllowed", &path));
            }
        }
        if let Some(a) = c.attachment {
            if !r.allowed_attachments.contains(attachment_kind(a)) {
                return Err(reject("AttachmentNotAllowed", &path));
            }
        }
    }
    for (slot, f) in &b.folded_blocks {
        for (i, prefix) in &f.prefixes {
            if !r.allowed_attachments.contains(condition_kind(*prefix)) {
                return Err(reject(
                    "AttachmentNotAllowed",
                    &format!("{path}.folded_blocks[{}].cells[{i}]", slot.get()),
                ));
            }
        }
        for (i, p) in f.cells.iter().enumerate() {
            validate_primary(r, *p, false).map_err(|mut e| {
                e.path = format!("{path}.folded_blocks[{}].cells[{i}]", slot.get());
                e
            })?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::BoardBounds;
    use codegrid_ir::{Cell, CustomDefinition, FoldedBlock, Program, IR_FORMAT_VERSION};
    use codegrid_model::Slot;
    use std::collections::{BTreeMap, BTreeSet};
    fn rules() -> ProgramRules {
        ProgramRules {
            allowed_instructions: instruction_identifiers()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            allowed_attachments: attachment_identifiers()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            main_board: BoardBounds {
                width: 10,
                height: 10,
            },
            function_board: BoardBounds {
                width: 10,
                height: 10,
            },
            max_functions: 10,
            max_custom: 10,
            max_threads: 10,
            memory_enabled: true,
        }
    }
    fn board(p: PrimaryInstruction) -> Board {
        Board {
            width: 2,
            height: 1,
            cells: vec![Cell::entry(Direction::Right), Cell::instruction(p, None)],
            folded_blocks: BTreeMap::new(),
        }
    }
    #[test]
    fn all_canonical_variants_and_independent_attachments() {
        let r = rules();
        for p in PrimaryInstruction::source_forms() {
            assert!(validate_primary(&r, Some(p), false).is_ok(), "{p:?}");
        }
        let mut r = r;
        r.allowed_instructions = BTreeSet::from(["READ_UP".into()]);
        assert!(validate_primary(&r, Some(PrimaryInstruction::Read(Direction::Up)), false).is_ok());
        assert!(
            validate_primary(&r, Some(PrimaryInstruction::Read(Direction::Down)), false).is_err()
        );
        let mut r = rules();
        r.allowed_attachments.clear();
        let mut b = board(PrimaryInstruction::Add);
        b.cells[1].attachment = Some(AttachmentInstruction::Repeat(2));
        assert_eq!(
            super::board(&r, &b, true, "main").unwrap_err().reason,
            "AttachmentNotAllowed"
        );
    }
    #[test]
    fn grouped_permissions_defaults_returns_and_generated_code() {
        let mut r = rules();
        r.allowed_instructions.clear();
        for p in PrimaryInstruction::source_forms() {
            let default = matches!(
                p,
                PrimaryInstruction::Direction(_)
                    | PrimaryInstruction::Output
                    | PrimaryInstruction::OutputImmediate(_)
                    | PrimaryInstruction::Halt
            );
            for generated in [false, true] {
                assert_eq!(
                    validate_primary(&r, Some(p), generated).is_ok(),
                    default,
                    "{p:?}, generated={generated}"
                );
            }
        }
        let slot = Slot::new(0).unwrap();
        let groups = [
            ("CMP", vec![PrimaryInstruction::Compare]),
            (
                "READ",
                vec![
                    PrimaryInstruction::Read(Direction::Up),
                    PrimaryInstruction::Read(Direction::Down),
                    PrimaryInstruction::Read(Direction::Left),
                    PrimaryInstruction::Read(Direction::Right),
                ],
            ),
            (
                "REGISTER_POINTER",
                vec![
                    PrimaryInstruction::MoveRegisterPointer(PointerDirection::Left),
                    PrimaryInstruction::MoveRegisterPointer(PointerDirection::Right),
                ],
            ),
            (
                "STACK",
                vec![PrimaryInstruction::Push, PrimaryInstruction::PopAdd],
            ),
            (
                "CODEC",
                vec![PrimaryInstruction::Decode, PrimaryInstruction::Encode],
            ),
            (
                "MEMORY",
                vec![
                    PrimaryInstruction::MemoryLoad,
                    PrimaryInstruction::MemoryStore,
                ],
            ),
            (
                "PAGE",
                vec![
                    PrimaryInstruction::MovePage(PageDirection::Increment),
                    PrimaryInstruction::MovePage(PageDirection::Decrement),
                ],
            ),
            (
                "SHIFT",
                vec![
                    PrimaryInstruction::Shift(ShiftDirection::Left),
                    PrimaryInstruction::Shift(ShiftDirection::Right),
                ],
            ),
            (
                "CALL",
                vec![PrimaryInstruction::Call(slot), PrimaryInstruction::Return],
            ),
            (
                "CUSTOM",
                vec![
                    PrimaryInstruction::Custom(slot),
                    PrimaryInstruction::CustomReturn,
                ],
            ),
        ];
        for (name, members) in groups {
            r.allowed_instructions = BTreeSet::from([name.into()]);
            for p in PrimaryInstruction::source_forms() {
                let default = matches!(
                    p,
                    PrimaryInstruction::Direction(_)
                        | PrimaryInstruction::Output
                        | PrimaryInstruction::OutputImmediate(_)
                        | PrimaryInstruction::Halt
                );
                let member = members.contains(&p)
                    || matches!(
                        (name, p),
                        ("CALL", PrimaryInstruction::Call(_))
                            | ("CUSTOM", PrimaryInstruction::Custom(_))
                    );
                for generated in [false, true] {
                    assert_eq!(
                        validate_primary(&r, Some(p), generated).is_ok(),
                        default || member,
                        "group={name}, {p:?}, generated={generated}"
                    );
                }
            }
        }
        for group in ["MEMORY", "PAGE"] {
            r.allowed_instructions = BTreeSet::from([group.into()]);
            r.memory_enabled = false;
            for p in [
                PrimaryInstruction::MemoryLoad,
                PrimaryInstruction::MemoryStore,
                PrimaryInstruction::MovePage(PageDirection::Increment),
                PrimaryInstruction::MovePage(PageDirection::Decrement),
            ] {
                assert_eq!(
                    validate_primary(&r, Some(p), true).unwrap_err().reason,
                    "GeneratedMemoryNotAllowed"
                );
            }
        }
    }
    #[test]
    fn folded_unreachable_and_custom_bodies_are_checked() {
        let mut r = rules();
        r.allowed_instructions.remove("ADD");
        let mut b = board(PrimaryInstruction::Halt);
        b.folded_blocks.insert(
            Slot::new(0).unwrap(),
            FoldedBlock {
                prefixes: Default::default(),
                cells: vec![Some(PrimaryInstruction::Add), None],
            },
        );
        assert_eq!(
            super::board(&r, &b, true, "main").unwrap_err().path,
            "main.folded_blocks[0].cells[0]"
        );
        let mut s = ScopedProgram {
            main: board(PrimaryInstruction::Halt),
            functions: BTreeMap::new(),
        };
        s.functions
            .insert(Slot::new(0).unwrap(), board(PrimaryInstruction::Add));
        assert!(scoped(&r, &s, "unused").is_err());
        let p = Program {
            format_version: IR_FORMAT_VERSION,
            outer: ScopedProgram {
                main: board(PrimaryInstruction::Halt),
                functions: BTreeMap::new(),
            },
            customs: BTreeMap::from([(Slot::new(0).unwrap(), CustomDefinition { program: s })]),
        };
        let verified = VerifiedProgram::new(p).unwrap();
        assert!(scoped(
            &r,
            &verified.program().customs[&Slot::new(0).unwrap()].program,
            "custom"
        )
        .is_err());
    }
    #[test]
    fn memory_generated_and_dimensions_threads_counts() {
        let mut r = rules();
        r.memory_enabled = false;
        for p in [
            PrimaryInstruction::MemoryLoad,
            PrimaryInstruction::MemoryStore,
            PrimaryInstruction::MovePage(PageDirection::Increment),
            PrimaryInstruction::MovePage(PageDirection::Decrement),
        ] {
            assert_eq!(
                validate_primary(&r, Some(p), true).unwrap_err().reason,
                "GeneratedMemoryNotAllowed"
            );
        }
        assert!(validate_primary(&r, None, true).is_ok());
        r.allowed_instructions.clear();
        assert_eq!(
            validate_primary(&r, Some(PrimaryInstruction::Add), true)
                .unwrap_err()
                .reason,
            "GeneratedInstructionNotAllowed"
        );
        let mut r = rules();
        r.main_board.width = 1;
        assert_eq!(
            super::board(&r, &board(PrimaryInstruction::Halt), true, "main")
                .unwrap_err()
                .reason,
            "BoardDimensionsExceeded"
        );
        let mut r = rules();
        r.max_threads = 1;
        let mut b = board(PrimaryInstruction::Halt);
        b.cells[1] = Cell::entry(Direction::Right);
        let s = ScopedProgram {
            main: b,
            functions: BTreeMap::new(),
        };
        assert_eq!(
            scoped(&r, &s, "outer").unwrap_err().reason,
            "ThreadCountExceeded"
        );
    }
    #[test]
    fn all_definition_counts_and_per_grid_thread_limit_use_verified_ir() {
        let slot = Slot::new(0).unwrap();
        let mut outer = ScopedProgram {
            main: board(PrimaryInstruction::Halt),
            functions: BTreeMap::new(),
        };
        outer
            .functions
            .insert(slot, board(PrimaryInstruction::Return));
        let custom = ScopedProgram {
            main: board(PrimaryInstruction::CustomReturn),
            functions: BTreeMap::from([(slot, board(PrimaryInstruction::Return))]),
        };
        let program = VerifiedProgram::new(Program {
            format_version: IR_FORMAT_VERSION,
            outer,
            customs: BTreeMap::from([(slot, CustomDefinition { program: custom })]),
        })
        .unwrap();
        let mut value: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/levels/echo.json")).unwrap();
        value["program_rules"]["allowed_instructions"] =
            serde_json::json!(["HALT", "RETURN", "CUSTOM_RETURN"]);
        value["program_rules"]["max_custom"] = serde_json::json!(1);
        value["program_rules"]["max_functions"] = serde_json::json!(1);
        let load = |value: &serde_json::Value| {
            crate::load_level_json(value.to_string().as_bytes(), 100_000).unwrap()
        };
        // Two independent code grids each have one Entry: no aggregate-thread restriction.
        assert!(validate_program(&load(&value), &program).is_ok());
        value["program_rules"]["max_custom"] = serde_json::json!(0);
        assert_eq!(
            validate_program(&load(&value), &program)
                .unwrap_err()
                .reason,
            "CustomCountExceeded"
        );
        value["program_rules"]["max_custom"] = serde_json::json!(1);
        value["program_rules"]["max_functions"] = serde_json::json!(0);
        assert_eq!(
            validate_program(&load(&value), &program)
                .unwrap_err()
                .reason,
            "FunctionCountExceeded"
        );
        let mut raw = program.program().clone();
        raw.outer.functions.clear();
        let custom_only = VerifiedProgram::new(raw).unwrap();
        assert_eq!(
            validate_program(&load(&value), &custom_only)
                .unwrap_err()
                .path,
            "customs[0]"
        );
        value["program_rules"]["max_functions"] = serde_json::json!(1);
        value["program_rules"]["function_board"]["width"] = serde_json::json!(1);
        assert_eq!(
            validate_program(&load(&value), &custom_only)
                .unwrap_err()
                .path,
            "customs[0].functions[0]"
        );
        value["program_rules"]["function_board"]["width"] = serde_json::json!(8);
        let mut raw = custom_only.program().clone();
        raw.customs.get_mut(&slot).unwrap().program.main.cells[1] = Cell::entry(Direction::Right);
        let threads = VerifiedProgram::new(raw).unwrap();
        assert_eq!(
            validate_program(&load(&value), &threads)
                .unwrap_err()
                .reason,
            "ThreadCountExceeded"
        );
    }

    #[test]
    fn conditional_permissions_are_independent_and_do_not_hide_primaries() {
        let mut r = rules();
        let mut b = board(PrimaryInstruction::Compare);
        b.cells[1].prefix = Some(codegrid_model::ConditionPrefix::One);
        r.allowed_attachments.clear();
        assert_eq!(
            super::board(&r, &b, true, "main").unwrap_err().reason,
            "AttachmentNotAllowed"
        );
        r.allowed_attachments.insert("CONDITION_0".into());
        assert!(super::board(&r, &b, true, "main").is_err());
        r.allowed_attachments.insert("CONDITION_1".into());
        assert!(super::board(&r, &b, true, "main").is_ok());
        r.allowed_instructions.remove("CMP");
        r.allowed_instructions.insert("STACK".into());
        assert_eq!(
            super::board(&r, &b, true, "main").unwrap_err().reason,
            "InstructionNotAllowed"
        );
        assert_eq!(
            validate_primary(&r, Some(PrimaryInstruction::Compare), true)
                .unwrap_err()
                .reason,
            "GeneratedInstructionNotAllowed"
        );
        let mut folded = board(PrimaryInstruction::Halt);
        folded.folded_blocks.insert(
            Slot::new(0).unwrap(),
            FoldedBlock {
                prefixes: BTreeMap::from([(1, codegrid_model::ConditionPrefix::Two)]),
                cells: vec![None, Some(PrimaryInstruction::Direction(Direction::Up))],
            },
        );
        let rejection = super::board(&r, &folded, true, "main").unwrap_err();
        assert_eq!(rejection.reason, "AttachmentNotAllowed");
        assert_eq!(rejection.path, "main.folded_blocks[0].cells[1]");
    }
}
