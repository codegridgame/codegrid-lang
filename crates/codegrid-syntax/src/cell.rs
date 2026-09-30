use codegrid_model::{AttachmentInstruction, Direction, PrimaryInstruction};

/// A fully matched Full source cell. Contextual placement rules are checked later.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CellToken {
    Empty,
    Entry(Direction),
    Instruction {
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
            primary,
            attachment: None,
        }),
        None => parse_attached_primary(token).ok_or(CellTokenError {
            code: "source.invalid_cell",
            message: "Unknown or malformed Full cell token, or an invalid Primary-Attachment combination.",
        }),
    }
}

fn parse_attached_primary(token: &str) -> Option<CellToken> {
    for attachment in AttachmentInstruction::ALL {
        let suffix = attachment.token();
        let Some(primary_token) = token.strip_suffix(&suffix) else {
            continue;
        };
        let primary = PrimaryInstruction::from_token(primary_token)?;
        if !primary.is_encodable() {
            return None;
        }
        if matches!(attachment, AttachmentInstruction::Repeat(_))
            && matches!(
                primary,
                PrimaryInstruction::Call(_) | PrimaryInstruction::Return
            )
        {
            return None;
        }
        return Some(CellToken::Instruction {
            primary,
            attachment: Some(attachment),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use codegrid_model::{AttachmentInstruction, Direction, PrimaryInstruction, ShiftDirection};

    use super::{parse_cell_token, CellToken};

    #[test]
    fn parses_complete_full_cells_and_attachments() {
        assert_eq!(parse_cell_token("_"), Ok(CellToken::Empty));
        assert_eq!(parse_cell_token("~^"), Ok(CellToken::Entry(Direction::Up)));
        assert_eq!(
            parse_cell_token("$<"),
            Ok(CellToken::Instruction {
                primary: PrimaryInstruction::Shift(ShiftDirection::Left),
                attachment: None,
            })
        );
        assert_eq!(
            parse_cell_token(",<"),
            Ok(CellToken::Instruction {
                primary: PrimaryInstruction::Read(Direction::Left),
                attachment: None,
            })
        );
        assert_eq!(
            parse_cell_token("$<x2"),
            Ok(CellToken::Instruction {
                primary: PrimaryInstruction::Shift(ShiftDirection::Left),
                attachment: Some(AttachmentInstruction::Repeat(2)),
            })
        );
        assert_eq!(
            parse_cell_token(",<*"),
            Ok(CellToken::Instruction {
                primary: PrimaryInstruction::Read(Direction::Left),
                attachment: Some(AttachmentInstruction::ReadCode),
            })
        );
        assert_eq!(
            parse_cell_token("+="),
            Ok(CellToken::Instruction {
                primary: PrimaryInstruction::Add,
                attachment: Some(AttachmentInstruction::WriteCode),
            })
        );
    }

    #[test]
    fn rejects_detached_and_malformed_complete_tokens() {
        for token in [
            "*", "=", "x2", "#0x3", "#00", "++", "+*x3", "+x2*", ";x2", ";*", ";=", "$0x2", "[0x3",
            "[0x2", "]x2", "#]x2", "~>x2", "~>*", "_x2", "[[0", "+x0", "+x6",
        ] {
            assert!(parse_cell_token(token).is_err(), "{token} must be rejected");
        }
    }

    #[test]
    fn rejects_each_standalone_primary_prefix_token() {
        for token in ["#", "$", ",", "[", "x"] {
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
                let compatible = primary.is_encodable()
                    && (!matches!(attachment, AttachmentInstruction::Repeat(_))
                        || !matches!(
                            primary,
                            PrimaryInstruction::Call(_) | PrimaryInstruction::Return
                        ));
                if compatible {
                    assert_eq!(
                        parse_cell_token(&token),
                        Ok(CellToken::Instruction {
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
}
