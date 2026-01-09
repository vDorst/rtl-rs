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
    pub fn new(i2cbus: &'bus mut I2C, addr: u8) -> Self {
        Self { i2c: i2cbus, addr }
    }

    pub fn read_reg(&mut self, reg: u16) -> Result<u32, I2C::Error> {
        let mut val = [0; 4];
        self.i2c
            .write_read(self.addr, reg.to_be_bytes().as_slice(), val.as_mut_slice())?;
        Ok(u32::from_le_bytes(val))
    }

    pub fn write_reg(&mut self, reg: u16, value: u32) -> Result<(), I2C::Error> {
        let mut val = [0; 6];
        val[0..2].copy_from_slice(reg.to_be_bytes().as_slice());
        val[2..6].copy_from_slice(&value.to_le_bytes());

        self.i2c.write(self.addr, &val)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::i2c::{Mock as I2cMock, Transaction as I2cTransaction};

    #[test]
    fn transaction() {
        let expect = vec![
            I2cTransaction::write_read(0x5c, vec![0xAB, 0xCD], vec![0x78, 0x56, 0x34, 0x12]),
            I2cTransaction::write(0x5c, vec![0xAB, 0xCD, 0x78, 0x56, 0x34, 0x12]),
        ];

        let mut i2cbus_mock = I2cMock::new(&expect);

        let mut rtl = Rtl837x::new(&mut i2cbus_mock, 0x5c);

        let val = rtl.read_reg(Regs::TestReg.into());
        assert_eq!(val, Ok(0x12345678));

        assert!(rtl.write_reg(Regs::TestReg.into(), 0x12345678).is_ok());

        i2cbus_mock.done();
    }
}
