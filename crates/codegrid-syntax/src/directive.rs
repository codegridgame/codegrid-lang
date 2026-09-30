use codegrid_model::{Slot, MAX_BOARD_CELLS, MAX_BOARD_DIMENSION};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CodeGridPath {
    Main,
    Custom(Slot),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoardPath {
    Main(CodeGridPath),
    Function { codegrid: CodeGridPath, slot: Slot },
}

/// Structural meaning of a definition directive, before lexical scope is
/// resolved by the compiler.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefinitionTarget {
    Main,
    Custom(Slot),
    RelativeFunction(Slot),
    RelativeFoldedBlock(Slot),
    Function { codegrid: CodeGridPath, slot: Slot },
    FoldedBlock { board: BoardPath, slot: Slot },
    RelativeFunctionFoldedBlock { function: Slot, slot: Slot },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DirectiveKind {
    Definition(DefinitionTarget),
    End,
    Size,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DirectiveNameError {
    MissingAtSign,
    InvalidPath,
    UnknownDirective,
}

impl DirectiveNameError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingAtSign => "source.missing_directive_prefix",
            Self::InvalidPath => "source.invalid_directive_path",
            Self::UnknownDirective => "source.unknown_directive",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BoardSize {
    pub width: usize,
    pub height: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SizeError {
    Malformed,
    Zero,
    Overflow,
}

impl SizeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Malformed => "source.invalid_size",
            Self::Zero => "source.zero_size",
            Self::Overflow => "source.geometry_limit",
        }
    }
}

/// Parses the normative `WIDTHxHEIGHT` spelling used by `@size`.
pub fn parse_size(value: &str) -> Result<BoardSize, SizeError> {
    let Some((width_text, height_text)) = value.split_once('x') else {
        return Err(SizeError::Malformed);
    };
    let width = parse_positive_integer(width_text)?;
    let height = parse_positive_integer(height_text)?;
    let cells = width.checked_mul(height).ok_or(SizeError::Overflow)?;
    if cells > MAX_BOARD_CELLS {
        return Err(SizeError::Overflow);
    }
    Ok(BoardSize {
        width: usize::try_from(width).map_err(|_| SizeError::Overflow)?,
        height: usize::try_from(height).map_err(|_| SizeError::Overflow)?,
    })
}

fn parse_positive_integer(value: &str) -> Result<u64, SizeError> {
    let bytes = value.as_bytes();
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return Err(SizeError::Malformed);
    }
    let parsed = value.parse::<u64>().map_err(|_| SizeError::Overflow)?;
    if parsed == 0 {
        return Err(SizeError::Zero);
    }
    if parsed > MAX_BOARD_DIMENSION {
        return Err(SizeError::Overflow);
    }
    Ok(parsed)
}

/// Parses a complete directive head such as `@main.F0.M2`.
///
/// Structural paths are case-insensitive. Instruction-token case rules are
/// handled separately by the model's exact instruction inventory.
pub fn parse_directive_head(head: &str) -> Result<DirectiveKind, DirectiveNameError> {
    let Some(path) = head.strip_prefix('@') else {
        return Err(DirectiveNameError::MissingAtSign);
    };
    if path.eq_ignore_ascii_case("size") {
        return Ok(DirectiveKind::Size);
    }
    if path.eq_ignore_ascii_case("end") {
        return Ok(DirectiveKind::End);
    }

    let segments: Vec<&str> = path.split('.').collect();
    if let Some(target) = parse_target(&segments) {
        return Ok(DirectiveKind::Definition(target));
    }

    let root = segments.first().copied().unwrap_or_default();
    if root.eq_ignore_ascii_case("main")
        || [b'C', b'F', b'M'].iter().any(|prefix| {
            root.as_bytes()
                .first()
                .is_some_and(|byte| byte.eq_ignore_ascii_case(prefix))
        })
    {
        Err(DirectiveNameError::InvalidPath)
    } else {
        Err(DirectiveNameError::UnknownDirective)
    }
}

fn parse_target(segments: &[&str]) -> Option<DefinitionTarget> {
    match segments {
        [main] if main.eq_ignore_ascii_case("main") => Some(DefinitionTarget::Main),
        [single] => {
            if let Some(slot) = parse_slot(single, b'C') {
                Some(DefinitionTarget::Custom(slot))
            } else if let Some(slot) = parse_slot(single, b'F') {
                Some(DefinitionTarget::RelativeFunction(slot))
            } else {
                parse_slot(single, b'M').map(DefinitionTarget::RelativeFoldedBlock)
            }
        }
        [main, function] if main.eq_ignore_ascii_case("main") => {
            if let Some(slot) = parse_slot(function, b'F') {
                Some(DefinitionTarget::Function {
                    codegrid: CodeGridPath::Main,
                    slot,
                })
            } else {
                parse_slot(function, b'M').map(|slot| DefinitionTarget::FoldedBlock {
                    board: BoardPath::Main(CodeGridPath::Main),
                    slot,
                })
            }
        }
        [first, second] => {
            if let (Some(function), Some(slot)) =
                (parse_slot(first, b'F'), parse_slot(second, b'M'))
            {
                Some(DefinitionTarget::RelativeFunctionFoldedBlock { function, slot })
            } else if let Some(custom) = parse_slot(first, b'C') {
                let codegrid = CodeGridPath::Custom(custom);
                if let Some(slot) = parse_slot(second, b'F') {
                    Some(DefinitionTarget::Function { codegrid, slot })
                } else {
                    parse_slot(second, b'M').map(|slot| DefinitionTarget::FoldedBlock {
                        board: BoardPath::Main(codegrid),
                        slot,
                    })
                }
            } else {
                None
            }
        }
        [main, function, folded] if main.eq_ignore_ascii_case("main") => {
            let function = parse_slot(function, b'F')?;
            let slot = parse_slot(folded, b'M')?;
            Some(DefinitionTarget::FoldedBlock {
                board: BoardPath::Function {
                    codegrid: CodeGridPath::Main,
                    slot: function,
                },
                slot,
            })
        }
        [custom, function, folded] => {
            let codegrid = CodeGridPath::Custom(parse_slot(custom, b'C')?);
            let function = parse_slot(function, b'F')?;
            let slot = parse_slot(folded, b'M')?;
            Some(DefinitionTarget::FoldedBlock {
                board: BoardPath::Function {
                    codegrid,
                    slot: function,
                },
                slot,
            })
        }
        _ => None,
    }
}

fn parse_slot(segment: &str, prefix: u8) -> Option<Slot> {
    let bytes = segment.as_bytes();
    if bytes.len() != 2 || !bytes[0].eq_ignore_ascii_case(&prefix) || !bytes[1].is_ascii_digit() {
        return None;
    }
    Slot::new(bytes[1] - b'0')
}

#[cfg(test)]
mod tests {
    use codegrid_model::Slot;

    use super::{
        parse_directive_head, parse_size, BoardPath, BoardSize, CodeGridPath, DefinitionTarget,
        DirectiveKind, DirectiveNameError, SizeError,
    };

    fn slot(value: u8) -> Slot {
        Slot::new(value).expect("test slot must be in range")
    }

    #[test]
    fn parses_case_insensitive_names_only_within_the_fixed_hierarchy() {
        let valid = [
            ("@MAIN", DefinitionTarget::Main),
            ("@c0", DefinitionTarget::Custom(slot(0))),
            ("@F9", DefinitionTarget::RelativeFunction(slot(9))),
            ("@M3", DefinitionTarget::RelativeFoldedBlock(slot(3))),
            (
                "@main.f0",
                DefinitionTarget::Function {
                    codegrid: CodeGridPath::Main,
                    slot: slot(0),
                },
            ),
            (
                "@C0.F0",
                DefinitionTarget::Function {
                    codegrid: CodeGridPath::Custom(slot(0)),
                    slot: slot(0),
                },
            ),
            (
                "@C0.M1",
                DefinitionTarget::FoldedBlock {
                    board: BoardPath::Main(CodeGridPath::Custom(slot(0))),
                    slot: slot(1),
                },
            ),
            (
                "@main.F0.M2",
                DefinitionTarget::FoldedBlock {
                    board: BoardPath::Function {
                        codegrid: CodeGridPath::Main,
                        slot: slot(0),
                    },
                    slot: slot(2),
                },
            ),
            (
                "@C0.F9.M9",
                DefinitionTarget::FoldedBlock {
                    board: BoardPath::Function {
                        codegrid: CodeGridPath::Custom(slot(0)),
                        slot: slot(9),
                    },
                    slot: slot(9),
                },
            ),
            (
                "@f0.M0",
                DefinitionTarget::RelativeFunctionFoldedBlock {
                    function: slot(0),
                    slot: slot(0),
                },
            ),
        ];
        for (head, target) in valid {
            assert_eq!(
                parse_directive_head(head),
                Ok(DirectiveKind::Definition(target)),
                "{head}"
            );
        }

        for head in [
            "@main.C0",
            "@C0.C1",
            "@M0.F0",
            "@main.F0.F1",
            "@C0.M0.F0",
            "@C10",
            "@F10",
            "@M10",
        ] {
            assert_eq!(
                parse_directive_head(head),
                Err(DirectiveNameError::InvalidPath),
                "{head}"
            );
        }
    }

    #[test]
    fn parses_directive_keywords_and_positive_dimensions() {
        assert_eq!(parse_directive_head("@SIZE"), Ok(DirectiveKind::Size));
        assert_eq!(parse_directive_head("@eNd"), Ok(DirectiveKind::End));
        assert_eq!(
            parse_size("5x3"),
            Ok(BoardSize {
                width: 5,
                height: 3
            })
        );
        assert_eq!(
            parse_size("005x03"),
            Ok(BoardSize {
                width: 5,
                height: 3
            })
        );
        assert_eq!(parse_size("0x3"), Err(SizeError::Zero));
        assert_eq!(parse_size("3x0"), Err(SizeError::Zero));
        assert_eq!(parse_size("000x3"), Err(SizeError::Zero));
        for malformed in ["3X5", "3x5x2", "3 x5", "x3"] {
            assert_eq!(
                parse_size(malformed),
                Err(SizeError::Malformed),
                "{malformed}"
            );
        }
    }

    #[test]
    fn rejects_dimensions_that_overflow_usize() {
        let width = format!("{}0", usize::MAX);
        let size = format!("{width}x1");

        assert_eq!(parse_size(&size), Err(SizeError::Overflow));
    }

    #[test]
    fn enforces_portable_dimension_and_total_cell_bounds() {
        assert_eq!(
            parse_size("4294967295x1"),
            Ok(BoardSize {
                width: u32::MAX as usize,
                height: 1,
            })
        );
        assert_eq!(parse_size("4294967296x1"), Err(SizeError::Overflow));
        assert_eq!(parse_size("65536x65536"), Err(SizeError::Overflow));
    }
}
