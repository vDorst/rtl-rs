mod regs;

use embedded_hal::i2c::I2c;

pub use crate::regs::Regs;

#[derive(thiserror::Error, Debug)]
pub enum Error<I> {
    #[error("embedded_hal Error")]
    Eh(I),
    #[error("Unknown chip id, found: {0:08x}")]
    UnknownChipID(u32),
}

pub struct Rtl837x<'bus, I> {
    i2c: &'bus mut I,
    addr: u8,
}

impl<'bus, I2C: I2c> Rtl837x<'bus, I2C> {
    pub fn new(i2cbus: &'bus mut I2C, addr: u8) -> Result<Self, I2C::Error> {
        // let mut id = [0;4];
        // i2cbus.write_read(addr, &regs::Regs::ChipID.as_slice(), id.as_mut_slice())?;
        // let id = u32::from_le_bytes(id);
        // if id != 0x83720000 {
        //     return Err(Error::UnknownChipID(id));
        // }
        Ok(Self { i2c: i2cbus, addr})
    }

    pub fn read_reg(&mut self, reg: Regs) -> Result<u32, I2C::Error> {
        let mut val = [0;4];
        self.i2c.write_read(self.addr, &reg.as_slice(), val.as_mut_slice())?;
        Ok(u32::from_le_bytes(val))
    }

    pub fn write_reg(&mut self, reg: Regs, value: u32) -> Result<(), I2C::Error> {
        let mut val = [0; 6];
        val[0..2].copy_from_slice(&reg.as_slice());
        val[2..4].copy_from_slice(&value.to_le_bytes());

        self.i2c.write(self.addr, &val)?;
        Ok(())
    }
}

