//! Host-independent values and the canonical instruction inventory for CodeGrid.
//!
//! This crate owns shared, host-independent language values and the canonical
//! textual instruction inventory. It contains no parser, VM, game rules, or
//! I/O.

/// The value stored in registers, stacks, input/output, and memory.
pub type Value = u8;

/// Portable source-level upper bound for either board dimension and for a
/// board's total cell count. Kept as `u64` so the limit is identical on native
/// and `wasm32` targets.
pub const MAX_BOARD_DIMENSION: u64 = u32::MAX as u64;
pub const MAX_BOARD_CELLS: u64 = u32::MAX as u64;

/// Program-wide boundary behavior for normal boards.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoundaryMode {
    Exit,
    Wrap,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PointerDirection {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PageDirection {
    Increment,
    Decrement,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ShiftDirection {
    Left,
    Right,
}

/// A validated structural ID used for Functions, Customs, and Folded Blocks.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Slot(u8);

impl Slot {
    pub const COUNT: usize = 10;

    pub const fn new(value: u8) -> Option<Self> {
        if value < Self::COUNT as u8 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// A Primary instruction value in the complete CodeGrid language.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PrimaryInstruction {
    Direction(Direction),
    RandomDirection,
    IfZero(Direction),
    Read(Direction),
    Clear,
    Add,
    Sub,
    MoveRegisterPointer(PointerDirection),
    Output,
    Push,
    PopAdd,
    Decode,
    Encode,
    Call(Slot),
    Return,
    Nand,
    MemoryLoad,
    MemoryStore,
    MovePage(PageDirection),
    Shift(ShiftDirection),
    FoldedBlock(Slot),
    Custom(Slot),
    CustomReturn,
    Halt,
}

impl PrimaryInstruction {
    /// The earlier restricted source inventory, retained temporarily while
    /// downstream crates are migrated to the complete language inventory.
    pub const MVP_SOURCE_TOKENS: [(&'static str, Self); 18] = [
        ("^", Self::Direction(Direction::Up)),
        ("v", Self::Direction(Direction::Down)),
        ("<", Self::Direction(Direction::Left)),
        (">", Self::Direction(Direction::Right)),
        ("#^", Self::IfZero(Direction::Up)),
        ("#v", Self::IfZero(Direction::Down)),
        ("#<", Self::IfZero(Direction::Left)),
        ("#>", Self::IfZero(Direction::Right)),
        (",^", Self::Read(Direction::Up)),
        (",v", Self::Read(Direction::Down)),
        (",<", Self::Read(Direction::Left)),
        (",>", Self::Read(Direction::Right)),
        (".", Self::Output),
        ("$<", Self::Shift(ShiftDirection::Left)),
        ("$>", Self::Shift(ShiftDirection::Right)),
        ("+", Self::Add),
        ("-", Self::Sub),
        (";", Self::Halt),
    ];

    /// Returns every Primary form accepted by the complete source language.
    pub fn source_forms() -> Vec<Self> {
        let mut forms = vec![
            Self::Direction(Direction::Up),
            Self::Direction(Direction::Down),
            Self::Direction(Direction::Left),
            Self::Direction(Direction::Right),
            Self::RandomDirection,
            Self::IfZero(Direction::Up),
            Self::IfZero(Direction::Down),
            Self::IfZero(Direction::Left),
            Self::IfZero(Direction::Right),
            Self::Read(Direction::Up),
            Self::Read(Direction::Down),
            Self::Read(Direction::Left),
            Self::Read(Direction::Right),
            Self::Clear,
            Self::Add,
            Self::Sub,
            Self::MoveRegisterPointer(PointerDirection::Left),
            Self::MoveRegisterPointer(PointerDirection::Right),
            Self::Output,
            Self::Push,
            Self::PopAdd,
            Self::Decode,
            Self::Encode,
            Self::Return,
            Self::Nand,
            Self::MemoryLoad,
            Self::MemoryStore,
            Self::MovePage(PageDirection::Increment),
            Self::MovePage(PageDirection::Decrement),
            Self::Shift(ShiftDirection::Left),
            Self::Shift(ShiftDirection::Right),
            Self::CustomReturn,
            Self::Halt,
        ];
        for value in 0..Slot::COUNT as u8 {
            let slot = Slot(value);
            forms.push(Self::Call(slot));
            forms.push(Self::FoldedBlock(slot));
            forms.push(Self::Custom(slot));
        }
        forms
    }

    /// Returns whether this value belongs to the earlier restricted profile.
    pub fn is_mvp(self) -> bool {
        Self::MVP_SOURCE_TOKENS
            .iter()
            .any(|(_, primary)| *primary == self)
    }

    /// Parses a complete canonical Primary token. Prefixes are never accepted
    /// on their own, and instruction spelling is case-sensitive.
    pub fn from_token(token: &str) -> Option<Self> {
        let primary = match token {
            "^" => Self::Direction(Direction::Up),
            "v" => Self::Direction(Direction::Down),
            "<" => Self::Direction(Direction::Left),
            ">" => Self::Direction(Direction::Right),
            "?" => Self::RandomDirection,
            "#^" => Self::IfZero(Direction::Up),
            "#v" => Self::IfZero(Direction::Down),
            "#<" => Self::IfZero(Direction::Left),
            "#>" => Self::IfZero(Direction::Right),
            ",^" => Self::Read(Direction::Up),
            ",v" => Self::Read(Direction::Down),
            ",<" => Self::Read(Direction::Left),
            ",>" => Self::Read(Direction::Right),
            "!" => Self::Clear,
            "+" => Self::Add,
            "-" => Self::Sub,
            "{" => Self::MoveRegisterPointer(PointerDirection::Left),
            "}" => Self::MoveRegisterPointer(PointerDirection::Right),
            "." => Self::Output,
            "(" => Self::Push,
            ")" => Self::PopAdd,
            "&" => Self::Decode,
            "%" => Self::Encode,
            "]" => Self::Return,
            "$&" => Self::Nand,
            "$(" => Self::MemoryLoad,
            "$)" => Self::MemoryStore,
            "$+" => Self::MovePage(PageDirection::Increment),
            "$-" => Self::MovePage(PageDirection::Decrement),
            "$<" => Self::Shift(ShiftDirection::Left),
            "$>" => Self::Shift(ShiftDirection::Right),
            "#]" => Self::CustomReturn,
            ";" => Self::Halt,
            _ => return parse_slot_instruction(token),
        };
        Some(primary)
    }

    /// Returns the source spelling without deriving it from an Instruction Code.
    pub fn token(self) -> String {
        match self {
            Self::Direction(Direction::Up) => "^".to_owned(),
            Self::Direction(Direction::Down) => "v".to_owned(),
            Self::Direction(Direction::Left) => "<".to_owned(),
            Self::Direction(Direction::Right) => ">".to_owned(),
            Self::RandomDirection => "?".to_owned(),
            Self::IfZero(Direction::Left) => "#<".to_owned(),
            Self::IfZero(Direction::Right) => "#>".to_owned(),
            Self::IfZero(Direction::Up) => "#^".to_owned(),
            Self::IfZero(Direction::Down) => "#v".to_owned(),
            Self::Read(Direction::Left) => ",<".to_owned(),
            Self::Read(Direction::Right) => ",>".to_owned(),
            Self::Read(Direction::Up) => ",^".to_owned(),
            Self::Read(Direction::Down) => ",v".to_owned(),
            Self::Clear => "!".to_owned(),
            Self::Add => "+".to_owned(),
            Self::Sub => "-".to_owned(),
            Self::MoveRegisterPointer(PointerDirection::Left) => "{".to_owned(),
            Self::MoveRegisterPointer(PointerDirection::Right) => "}".to_owned(),
            Self::Output => ".".to_owned(),
            Self::Push => "(".to_owned(),
            Self::PopAdd => ")".to_owned(),
            Self::Decode => "&".to_owned(),
            Self::Encode => "%".to_owned(),
            Self::Call(slot) => format!("[{}", slot.get()),
            Self::Return => "]".to_owned(),
            Self::Nand => "$&".to_owned(),
            Self::MemoryLoad => "$(".to_owned(),
            Self::MemoryStore => "$)".to_owned(),
            Self::MovePage(PageDirection::Increment) => "$+".to_owned(),
            Self::MovePage(PageDirection::Decrement) => "$-".to_owned(),
            Self::Shift(ShiftDirection::Left) => "$<".to_owned(),
            Self::Shift(ShiftDirection::Right) => "$>".to_owned(),
            Self::FoldedBlock(slot) => format!("${}", slot.get()),
            Self::Custom(slot) => format!("#{}", slot.get()),
            Self::CustomReturn => "#]".to_owned(),
            Self::Halt => ";".to_owned(),
        }
    }

    pub const fn is_encodable(self) -> bool {
        !matches!(
            self,
            Self::FoldedBlock(_) | Self::Custom(_) | Self::CustomReturn | Self::Halt
        )
    }

    /// Returns the normative encoding for encodable Primary forms.
    pub const fn instruction_code(self) -> Option<u8> {
        match self {
            Self::Direction(Direction::Up) => Some(94),
            Self::Direction(Direction::Down) => Some(118),
            Self::Direction(Direction::Left) => Some(60),
            Self::Direction(Direction::Right) => Some(62),
            Self::RandomDirection => Some(63),
            Self::IfZero(Direction::Left) => Some(95),
            Self::IfZero(Direction::Right) => Some(97),
            Self::IfZero(Direction::Up) => Some(129),
            Self::IfZero(Direction::Down) => Some(153),
            Self::Read(Direction::Left) => Some(104),
            Self::Read(Direction::Right) => Some(106),
            Self::Read(Direction::Up) => Some(138),
            Self::Read(Direction::Down) => Some(162),
            Self::Clear => Some(33),
            Self::Add => Some(43),
            Self::Sub => Some(45),
            Self::MoveRegisterPointer(PointerDirection::Left) => Some(123),
            Self::MoveRegisterPointer(PointerDirection::Right) => Some(125),
            Self::Output => Some(46),
            Self::Push => Some(40),
            Self::PopAdd => Some(41),
            Self::Decode => Some(38),
            Self::Encode => Some(37),
            Self::Call(slot) => Some(139 + slot.get()),
            Self::Return => Some(93),
            Self::Nand => Some(74),
            Self::MemoryLoad => Some(76),
            Self::MemoryStore => Some(77),
            Self::MovePage(PageDirection::Increment) => Some(79),
            Self::MovePage(PageDirection::Decrement) => Some(81),
            Self::Shift(ShiftDirection::Left) => Some(96),
            Self::Shift(ShiftDirection::Right) => Some(98),
            Self::FoldedBlock(_) | Self::Custom(_) | Self::CustomReturn | Self::Halt => None,
        }
    }

    /// Decodes a byte only when it is a normative encodable Primary code.
    /// EMPTY is represented separately by [`InstructionStackItem`].
    pub fn from_instruction_code(code: u8) -> Option<Self> {
        let primary = match code {
            33 => Self::Clear,
            37 => Self::Encode,
            38 => Self::Decode,
            40 => Self::Push,
            41 => Self::PopAdd,
            43 => Self::Add,
            45 => Self::Sub,
            46 => Self::Output,
            60 => Self::Direction(Direction::Left),
            62 => Self::Direction(Direction::Right),
            63 => Self::RandomDirection,
            74 => Self::Nand,
            76 => Self::MemoryLoad,
            77 => Self::MemoryStore,
            79 => Self::MovePage(PageDirection::Increment),
            81 => Self::MovePage(PageDirection::Decrement),
            93 => Self::Return,
            94 => Self::Direction(Direction::Up),
            95 => Self::IfZero(Direction::Left),
            96 => Self::Shift(ShiftDirection::Left),
            97 => Self::IfZero(Direction::Right),
            98 => Self::Shift(ShiftDirection::Right),
            104 => Self::Read(Direction::Left),
            106 => Self::Read(Direction::Right),
            118 => Self::Direction(Direction::Down),
            123 => Self::MoveRegisterPointer(PointerDirection::Left),
            125 => Self::MoveRegisterPointer(PointerDirection::Right),
            129 => Self::IfZero(Direction::Up),
            138 => Self::Read(Direction::Up),
            139..=148 => Self::Call(Slot::new(code - 139)?),
            153 => Self::IfZero(Direction::Down),
            162 => Self::Read(Direction::Down),
            _ => return None,
        };
        Some(primary)
    }
}

fn parse_slot_instruction(token: &str) -> Option<PrimaryInstruction> {
    let bytes = token.as_bytes();
    if bytes.len() != 2 || !bytes[1].is_ascii_digit() {
        return None;
    }
    let slot = Slot::new(bytes[1] - b'0')?;
    match bytes[0] {
        b'[' => Some(PrimaryInstruction::Call(slot)),
        b'$' => Some(PrimaryInstruction::FoldedBlock(slot)),
        b'#' => Some(PrimaryInstruction::Custom(slot)),
        _ => None,
    }
}

/// A value that may be stored on an Instruction Stack.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum InstructionStackItem {
    Empty,
    Primary(EncodablePrimary),
}

/// A Primary value that has a valid v2 Instruction Code.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EncodablePrimary {
    primary: PrimaryInstruction,
    code: u8,
}

impl EncodablePrimary {
    pub fn new(primary: PrimaryInstruction) -> Option<Self> {
        Some(Self {
            primary,
            code: primary.instruction_code()?,
        })
    }

    pub const fn primary(self) -> PrimaryInstruction {
        self.primary
    }

    pub const fn code(self) -> u8 {
        self.code
    }
}

impl InstructionStackItem {
    /// Decodes a valid Primary code or EMPTY; all other bytes are invalid.
    pub fn from_code(code: u8) -> Option<Self> {
        if code == 32 {
            Some(Self::Empty)
        } else {
            PrimaryInstruction::from_instruction_code(code)
                .and_then(EncodablePrimary::new)
                .map(Self::Primary)
        }
    }

    /// Constructs an Instruction Stack item from an encodable Primary.
    pub fn from_primary(primary: PrimaryInstruction) -> Option<Self> {
        EncodablePrimary::new(primary).map(Self::Primary)
    }

    /// Returns the normative byte encoding of this stack item.
    pub const fn code(self) -> u8 {
        match self {
            Self::Empty => 32,
            Self::Primary(primary) => primary.code(),
        }
    }

    pub const fn primary(self) -> Option<PrimaryInstruction> {
        match self {
            Self::Empty => None,
            Self::Primary(primary) => Some(primary.primary()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AttachmentInstruction {
    ReadCode,
    WriteCode,
    Repeat(u8),
}

impl AttachmentInstruction {
    pub const ALL: [Self; 6] = [
        Self::ReadCode,
        Self::WriteCode,
        Self::Repeat(2),
        Self::Repeat(3),
        Self::Repeat(4),
        Self::Repeat(5),
    ];

    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "*" => Some(Self::ReadCode),
            "=" => Some(Self::WriteCode),
            "x2" => Some(Self::Repeat(2)),
            "x3" => Some(Self::Repeat(3)),
            "x4" => Some(Self::Repeat(4)),
            "x5" => Some(Self::Repeat(5)),
            _ => None,
        }
    }

    pub fn token(self) -> String {
        match self {
            Self::ReadCode => "*".to_owned(),
            Self::WriteCode => "=".to_owned(),
            Self::Repeat(count) => format!("x{count}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AttachmentInstruction, InstructionStackItem, PrimaryInstruction, Slot};

    #[test]
    fn board_geometry_limits_are_portable_u32_bounds() {
        assert_eq!(super::MAX_BOARD_DIMENSION, u32::MAX as u64);
        assert_eq!(super::MAX_BOARD_CELLS, u32::MAX as u64);
    }

    #[test]
    fn slot_rejects_out_of_range_ids() {
        assert_eq!(Slot::new(9).map(Slot::get), Some(9));
        assert_eq!(Slot::new(10), None);
    }

    #[test]
    fn instruction_stack_codes_round_trip() {
        let valid_codes = [
            32, 33, 37, 38, 40, 41, 43, 45, 46, 60, 62, 63, 74, 76, 77, 79, 81, 93, 94, 95, 96, 97,
            98, 104, 106, 118, 123, 125, 129, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147,
            148, 153, 162,
        ];

        for code in valid_codes {
            let item =
                InstructionStackItem::from_code(code).expect("listed instruction code must decode");
            assert_eq!(item.code(), code);
        }
    }

    #[test]
    fn encodable_primary_codes_round_trip_without_duplicate_codes() {
        let mut codes = std::collections::BTreeSet::new();

        for primary in PrimaryInstruction::source_forms() {
            let Some(code) = primary.instruction_code() else {
                assert!(!primary.is_encodable(), "{} has no code", primary.token());
                continue;
            };
            assert!(codes.insert(code), "duplicate Instruction Code {code}");
            assert_eq!(
                PrimaryInstruction::from_instruction_code(code),
                Some(primary),
                "Instruction Code {code} must decode to {}",
                primary.token()
            );
        }
    }

    #[test]
    fn decoder_rejects_attachment_prefix_and_structural_codes() {
        for code in [35, 36, 42, 44, 59, 61, 70, 91, 120] {
            assert_eq!(InstructionStackItem::from_code(code), None);
        }
        assert_eq!(PrimaryInstruction::from_instruction_code(32), None,);
    }

    #[test]
    fn structural_primary_forms_are_not_encodable() {
        for value in 0..Slot::COUNT as u8 {
            let slot = Slot::new(value).expect("value is within the slot range");
            for primary in [
                PrimaryInstruction::FoldedBlock(slot),
                PrimaryInstruction::Custom(slot),
            ] {
                assert!(!primary.is_encodable(), "{}", primary.token());
                assert_eq!(primary.instruction_code(), None, "{}", primary.token());
                assert_eq!(InstructionStackItem::from_primary(primary), None);
            }
        }

        for primary in [PrimaryInstruction::CustomReturn, PrimaryInstruction::Halt] {
            assert!(!primary.is_encodable(), "{}", primary.token());
            assert_eq!(primary.instruction_code(), None);
            assert_eq!(InstructionStackItem::from_primary(primary), None);
        }
    }

    #[test]
    fn complete_primary_tokens_round_trip_without_duplicates() {
        let forms = PrimaryInstruction::source_forms();
        assert_eq!(forms.len(), 63);
        let mut tokens = std::collections::BTreeSet::new();

        for form in forms {
            let token = form.token();
            assert!(
                tokens.insert(token.clone()),
                "duplicate source form: {token}"
            );
            assert_eq!(PrimaryInstruction::from_token(&token), Some(form));
        }

        for token in [
            "#", "$", ",", "[", "x2", "[00", "[10", "$10", "#00", "#10", "*", "=", "x6", "+x2",
            "<*",
        ] {
            assert_eq!(
                PrimaryInstruction::from_token(token),
                None,
                "{token} is not a complete Primary token"
            );
        }
    }

    #[test]
    fn every_attachment_has_one_exact_source_token() {
        assert_eq!(AttachmentInstruction::ALL.len(), 6);
        for attachment in AttachmentInstruction::ALL {
            let token = attachment.token();
            assert_eq!(AttachmentInstruction::from_token(&token), Some(attachment));
            assert_eq!(PrimaryInstruction::from_token(&token), None);
        }
        for token in ["x0", "x1", "x6", "x10", "x2x3", "*x2"] {
            assert_eq!(AttachmentInstruction::from_token(token), None, "{token}");
        }
    }
}
