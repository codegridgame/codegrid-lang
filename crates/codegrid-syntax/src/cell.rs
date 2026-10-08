use codegrid_model::{AttachmentInstruction, ConditionPrefix, Direction, PrimaryInstruction};

/// A fully matched Full source cell. Contextual placement rules are checked later.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CellToken {
    Empty,
    Entry(Direction),
    Instruction {
        prefix: Option<ConditionPrefix>,
        primary: PrimaryInstruction,
        attachment: Option<AttachmentInstruction>,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CellTokenError {
    pub code: &'static str,
    pub message: &'static str,
}

/// Parses one complete Full cell token without splitting an invalid Primary
/// into a valid prefix and ignored suffix.
pub fn parse_cell_token(token: &str) -> Result<CellToken, CellTokenError> {
    for prefix in ConditionPrefix::ALL {
        if let Some(rest) = token.strip_prefix(prefix.token()) {
            return match parse_unprefixed_cell(rest) {
                Ok(CellToken::Instruction {
                    prefix: None,
                    primary,
                    attachment,
                }) => Ok(CellToken::Instruction {
                    prefix: Some(prefix),
                    primary,
                    attachment,
                }),
                Err(error) => Err(CellTokenError {
                    code: "source.invalid_cell",
                    message: error.message,
                }),
                _ => Err(CellTokenError {
                    code: "source.invalid_cell",
                    message: "A conditional prefix cannot apply to an Empty cell or Entry marker; follow it with one Primary instruction.",
                }),
            };
        }
    }
    parse_unprefixed_cell(token)
}

fn parse_unprefixed_cell(token: &str) -> Result<CellToken, CellTokenError> {
    if token == "_" {
        return Ok(CellToken::Empty);
    }

    let entry = match token {
        "~^" => Some(Direction::Up),
        "~v" => Some(Direction::Down),
        "~<" => Some(Direction::Left),
        "~>" => Some(Direction::Right),
        _ => None,
    };
    if let Some(direction) = entry {
        return Ok(CellToken::Entry(direction));
    }

    if token.starts_with('~') {
        return Err(CellTokenError {
            code: "source.invalid_entry",
            message: "Entry markers must be exactly one of ~^, ~v, ~<, or ~> and cannot have an Attachment.",
        });
    }

    match PrimaryInstruction::from_token(token) {
        Some(primary) => Ok(CellToken::Instruction {
            prefix: None,
            primary,
            attachment: None,
        }),
        None => parse_attached_primary(token),
    }
}

fn parse_attached_primary(token: &str) -> Result<CellToken, CellTokenError> {
    for attachment in AttachmentInstruction::ALL {
        let suffix = attachment.token();
        let Some(primary_token) = token.strip_suffix(&suffix) else {
            continue;
        };
        let Some(primary) = PrimaryInstruction::from_token(primary_token) else {
            continue;
        };
        let immediate_repeat = matches!(primary, PrimaryInstruction::OutputImmediate(_))
            && matches!(attachment, AttachmentInstruction::Repeat(_));
        if !primary.is_encodable() && !immediate_repeat {
            return Err(CellTokenError {
                code: "source.invalid_cell",
                message: match primary {
                    PrimaryInstruction::OutputImmediate(_) => "Immediate output .0 through .9 allows only Repeat x2 through x5; ReadCode (*) and WriteCode (=) are not allowed.",
                    PrimaryInstruction::Halt => "HALT (;) cannot carry a suffix Attachment.",
                    PrimaryInstruction::FoldedBlock(_) => "A Folded Block call cannot carry a suffix Attachment.",
                    PrimaryInstruction::Custom(_) | PrimaryInstruction::CustomReturn => "A Custom call or return cannot carry a suffix Attachment.",
                    _ => "This Primary instruction cannot carry a suffix Attachment.",
                },
            });
        }
        if matches!(attachment, AttachmentInstruction::Repeat(_))
            && matches!(
                primary,
                PrimaryInstruction::Call(_) | PrimaryInstruction::Return
            )
        {
            return Err(CellTokenError {
                code: "source.invalid_cell",
                message: "Repeat is not allowed on Function CALL ([0 through [9) or RETURN (]); use an ordinary call/return without x2 through x5.",
            });
        }
        return Ok(CellToken::Instruction {
            prefix: None,
            primary,
            attachment: Some(attachment),
        });
    }
    Err(CellTokenError {
        code: "source.invalid_cell",
        message: invalid_token_message(token),
    })
}

/// Refine rejected atoms for diagnostics only; never recover executable cells.
fn invalid_token_message(token: &str) -> &'static str {
    if ConditionPrefix::ALL
        .iter()
        .any(|prefix| token.starts_with(prefix.token()))
    {
        return "A cell allows only one conditional prefix (?0, ?1, ?2, or ?!); nested or repeated prefixes are not allowed.";
    }
    if token == "*" || token == "=" || token.starts_with('x') {
        return "A suffix Attachment must be joined directly to a Primary in the same cell; it cannot stand alone.";
    }
    if token.starts_with('_') {
        return "Empty cells must be exactly _ and cannot carry a prefix or suffix Attachment.";
    }
    if token.starts_with('?') && token.as_bytes().get(1).is_some_and(u8::is_ascii_digit) {
        return "A conditional prefix must be ?0, ?1, ?2, or ?! followed directly by one Primary instruction.";
    }
    if token.starts_with(',') {
        return "READ is the single token , without a direction; use at most one allowed suffix Attachment.";
    }
    for attachment in AttachmentInstruction::ALL {
        if let Some(base) = token.strip_suffix(&attachment.token()) {
            if AttachmentInstruction::ALL
                .iter()
                .any(|earlier| base.ends_with(&earlier.token()))
            {
                return "A cell allows only one suffix Attachment; multiple or combined suffixes are not allowed.";
            }
        }
    }
    if let Some((primary, count)) = token.rsplit_once('x') {
        if PrimaryInstruction::from_token(primary).is_some() {
            return if !count.is_empty() && count.bytes().all(|byte| byte.is_ascii_digit()) {
                "Repeat count must be a single digit from 2 through 5; use x2, x3, x4, or x5."
            } else {
                "Malformed Repeat suffix; use x2, x3, x4, or x5 immediately after the Primary."
            };
        }
    }
    if token.starts_with('.') && token != "." {
        return "Immediate output must be .0 through .9 with exactly one ASCII digit; only an optional Repeat suffix x2 through x5 is allowed.";
    }
    if token.starts_with('[') || token.starts_with('$') || token.starts_with('#') {
        return "Unknown or malformed Primary reference; Function CALL uses [0 through [9, Folded Block CALL uses $0 through $9, and Custom CALL uses #0 through #9 or #] for return.";
    }
    "Unknown or malformed Full cell token; use one complete Primary instruction with at most one conditional prefix and one allowed suffix Attachment."
}

#[cfg(test)]
mod tests {
    use codegrid_model::{
        AttachmentInstruction, ConditionPrefix, Direction, PrimaryInstruction, ShiftDirection,
    };

    use super::{parse_cell_token, CellToken};

    #[test]
    fn parses_complete_full_cells_and_attachments() {
        assert_eq!(parse_cell_token("_"), Ok(CellToken::Empty));
        assert_eq!(parse_cell_token("~^"), Ok(CellToken::Entry(Direction::Up)));
        assert_eq!(
            parse_cell_token("$<"),
            Ok(CellToken::Instruction {
                prefix: None,
                primary: PrimaryInstruction::Shift(ShiftDirection::Left),
                attachment: None,
            })
        );
        assert_eq!(
            parse_cell_token(","),
            Ok(CellToken::Instruction {
                prefix: None,
                primary: PrimaryInstruction::Read,
                attachment: None,
            })
        );
        assert_eq!(
            parse_cell_token("$<x2"),
            Ok(CellToken::Instruction {
                prefix: None,
                primary: PrimaryInstruction::Shift(ShiftDirection::Left),
                attachment: Some(AttachmentInstruction::Repeat(2)),
            })
        );
        assert_eq!(
            parse_cell_token(",*"),
            Ok(CellToken::Instruction {
                prefix: None,
                primary: PrimaryInstruction::Read,
                attachment: Some(AttachmentInstruction::ReadCode),
            })
        );
        assert_eq!(
            parse_cell_token("+="),
            Ok(CellToken::Instruction {
                prefix: None,
                primary: PrimaryInstruction::Add,
                attachment: Some(AttachmentInstruction::WriteCode),
            })
        );
    }

    #[test]
    fn rejects_detached_and_malformed_complete_tokens() {
        for token in [
            ",^", ",v", ",<", ",>", "?!,v", ",^x2", "*", "=", "x2", "#0x3", "#00", "++", "+*x3",
            "+x2*", ";x2", ";*", ";=", "$0x2", "[0x3", "[0x2", "]x2", "#]x2", "~>x2", "~>*", "_x2",
            "[[0", "+x0", "+x6",
        ] {
            assert!(parse_cell_token(token).is_err(), "{token} must be rejected");
        }
    }

    #[test]
    fn rejects_each_standalone_primary_prefix_token() {
        for token in ["#", "$", "[", "x"] {
            assert!(
                parse_cell_token(token).is_err(),
                "standalone token {token:?} must be rejected"
            );
        }
    }

    #[test]
    fn accepts_every_full_primary_as_one_complete_cell_token() {
        for primary in PrimaryInstruction::source_forms() {
            let token = primary.token();
            assert_eq!(
                parse_cell_token(&token),
                Ok(CellToken::Instruction {
                    prefix: None,
                    primary,
                    attachment: None,
                }),
                "{token}"
            );
        }
    }

    #[test]
    fn accepts_read_write_code_and_repeat_according_to_the_encoding_matrix() {
        for primary in PrimaryInstruction::source_forms() {
            for attachment in AttachmentInstruction::ALL {
                let token = format!("{}{}", primary.token(), attachment.token());
                let compatible = (primary.is_encodable()
                    || (matches!(primary, PrimaryInstruction::OutputImmediate(_))
                        && matches!(attachment, AttachmentInstruction::Repeat(_))))
                    && (!matches!(attachment, AttachmentInstruction::Repeat(_))
                        || !matches!(
                            primary,
                            PrimaryInstruction::Call(_) | PrimaryInstruction::Return
                        ));
                if compatible {
                    assert_eq!(
                        parse_cell_token(&token),
                        Ok(CellToken::Instruction {
                            prefix: None,
                            primary,
                            attachment: Some(attachment),
                        }),
                        "{token} must be accepted"
                    );
                } else {
                    assert!(
                        parse_cell_token(&token).is_err(),
                        "{token} must be rejected"
                    );
                }
            }
        }
    }

    #[test]
    fn rejects_multiple_attachments() {
        for token in ["+*x2", "+=x2", "+x2*", "+x2x3", "*x2"] {
            assert!(parse_cell_token(token).is_err(), "{token} must be rejected");
        }
    }

    #[test]
    fn prefixes_cover_every_primary_and_reject_detached_or_nested_forms() {
        for prefix in ConditionPrefix::ALL {
            for primary in PrimaryInstruction::source_forms() {
                let token = format!("{}{}", prefix.token(), primary.token());
                assert!(
                    matches!(parse_cell_token(&token), Ok(CellToken::Instruction { prefix: Some(actual), primary: actual_primary, .. }) if actual == prefix && actual_primary == primary),
                    "{token}"
                );
            }
        }
        for token in [
            "?", "#^", "#v", "#<", "#>", "?0", "?1", "?2", "?3+", "?0_", "?0~>", "?0?1+", "?0?0+",
            "?0+*=", "?0.3*", "?0[0x2",
        ] {
            assert!(parse_cell_token(token).is_err(), "{token}");
        }
        assert!(parse_cell_token(&format!("{}+", "?0".repeat(100_000))).is_err());
        for token in ["?=", "?==", "?=*", "?=x3", "?2?==", "?0??*"] {
            assert!(parse_cell_token(token).is_ok(), "{token}");
        }
    }
}
