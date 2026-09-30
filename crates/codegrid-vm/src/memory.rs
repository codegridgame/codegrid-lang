use std::collections::BTreeMap;

use num_bigint::BigInt;

/// Arbitrary-precision signed page and memory address types.
pub type Page = BigInt;
pub type MemoryAddress = BigInt;

/// Computes `Page * 256 + offset` without integer wraparound.
pub fn effective_address(page: &Page, offset: u8) -> MemoryAddress {
    page.clone() * BigInt::from(256u16) + BigInt::from(offset)
}

/// Sparse zero-initialized memory shared by threads in one execution context.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Memory {
    cells: BTreeMap<MemoryAddress, u8>,
}

impl Memory {
    pub fn from_cells(cells: BTreeMap<MemoryAddress, u8>) -> Self {
        Self {
            cells: cells.into_iter().filter(|(_, value)| *value != 0).collect(),
        }
    }

    pub fn read(&self, address: &MemoryAddress) -> u8 {
        self.cells.get(address).copied().unwrap_or(0)
    }

    pub fn write(&mut self, address: MemoryAddress, value: u8) {
        if value == 0 {
            self.cells.remove(&address);
        } else {
            self.cells.insert(address, value);
        }
    }

    pub fn allocated_addresses(&self) -> impl Iterator<Item = &MemoryAddress> {
        self.cells.keys()
    }

    pub fn allocated_cells(&self) -> usize {
        self.cells.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{effective_address, Memory, Page};
    use num_bigint::BigInt;

    #[test]
    fn computes_positive_and_negative_addresses() {
        assert_eq!(effective_address(&Page::from(0), 0), BigInt::from(0));
        assert_eq!(effective_address(&Page::from(0), 255), BigInt::from(255));
        assert_eq!(effective_address(&Page::from(1), 0), BigInt::from(256));
        assert_eq!(effective_address(&Page::from(-1), 255), BigInt::from(-1));
        assert_eq!(effective_address(&Page::from(-1), 0), BigInt::from(-256));
    }

    #[test]
    fn supports_addresses_beyond_machine_integer_ranges() {
        let page = BigInt::from(1u8) << 256usize;
        let address = effective_address(&page, 17);
        let mut memory = Memory::default();

        assert_eq!(memory.read(&address), 0);
        memory.write(address.clone(), 231);
        assert_eq!(memory.read(&address), 231);
        assert_eq!(memory.allocated_cells(), 1);
    }

    #[test]
    fn zero_writes_restore_sparse_default_state() {
        let address = BigInt::from(-17);
        let mut memory = Memory::default();
        memory.write(address.clone(), 9);
        memory.write(address.clone(), 0);

        assert_eq!(memory.read(&address), 0);
        assert_eq!(memory.allocated_cells(), 0);
    }
}
