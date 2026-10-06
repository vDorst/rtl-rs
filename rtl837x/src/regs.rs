#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum Regs {
    ChipID = 0x0004,
    ChipInfo = 0x000c,

    Gpio00_31Output = 0x3c,
    Gpio32_63Output = 0x40,

    Gpio00_31Input = 0x44,
    Gpio32_63Input = 0x48,

    Gpio00_31Dir = 0x4c,
    Gpio32_63Dir = 0x50,

    SdsIndacsCmd = 0x3F8,
    SdsIndacsRd = 0x3FC,
    SdsIndacsWd = 0x400,

    I2cMasterCtrl = 0x414,
    I2cMst1Ctrl1 = 0x418,
    I2cMst1Ctrl2 = 0x41c,
    I2cAddrData = 0x420,
    I2cData = 0x424,

    SdsModeSel = 0x7B20,
    IoMuxSel0 = 0x7F8C,
    IoMuxSel1 = 0x7F90,
    IoMuxSel2 = 0x7F94,

    SmiAccessPhyCtrl0 = 0x6438,
    SmiAccessPhyCtrl1 = 0x643C,
    SmiAccessPhyCtrl2 = 0x6440,
    SmiAccessPhyCtrl3 = 0x6444,

    LedGlbMux1 = 0x65e0,
    LedGlbMux2 = 0x65e4,
    LedGlbMux3 = 0x65e8,
    LedGlbMux4 = 0x65ec,
    LedGlbMux5 = 0x65f0,
    LedGlbMux6 = 0x65f4,

    MacLinkStatus = 0x63e8,

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
