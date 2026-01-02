#[derive(Debug)]
#[repr(u16)]
pub enum Regs {
    ChipID = 0x0004,
}

impl Regs {
    pub fn as_slice(self) -> [u8; 2] {
        (self as u16).to_be_bytes()
    }
}
