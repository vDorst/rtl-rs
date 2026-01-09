#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum Regs {
    ChipID = 0x0004,

    IoMuxSel0 = 0x7F8C,
    IoMuxSel1 = 0x7F90,
    IoMuxSel2 = 0x7F94,
    #[cfg(test)]
    TestReg = 0xABCD,
}

impl From<Regs> for u16 {
    fn from(value: Regs) -> Self {
        value as _
    }
}

impl Regs {
    pub fn as_slice(self) -> [u8; 2] {
        (self as u16).to_be_bytes()
    }
}
