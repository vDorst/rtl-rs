#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum Regs {
    ChipID = 0x0004,
    ChipInfo = 0x000c,

    SdsIndacsCmd = 0x3F8,
    SdsIndacsRd = 0x3FC,
    SdsIndacsWd = 0x400,

    SdsModeSel = 0x7B20,
    IoMuxSel0 = 0x7F8C,
    IoMuxSel1 = 0x7F90,
    IoMuxSel2 = 0x7F94,

    SmiAccessPhyCtrl0 = 0x6438,
    SmiAccessPhyCtrl1 = 0x643C,
    SmiAccessPhyCtrl2 = 0x6440,
    SmiAccessPhyCtrl3 = 0x6444,

    I2cMst1Ctrl1 = 0x418,
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
