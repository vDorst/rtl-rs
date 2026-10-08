mod i2c;
mod regs;

use embedded_hal::i2c::I2c;

pub use crate::i2c::{I2C_SCL, I2C_SDA, phy_modify, phy_read, phy_write};
pub use crate::regs::Regs;

#[derive(thiserror::Error, Debug)]
pub enum Error<I> {
    #[error("embedded_hal Error")]
    Eh(I),
    #[error("Unknown chip id, found: {0:08x}")]
    UnknownChipID(u32),
    #[error("Invalue user mask {0}")]
    UserMask(u32),
    #[error("SMI Access TimeOut")]
    PhyAccessTimeOut,
    #[error("Function Input invalid")]
    InvalidInput,
    #[error("I2C Nack")]
    I2CNack,
}

impl<I> From<I> for Error<I>
where
    I: embedded_hal::i2c::Error,
{
    fn from(error: I) -> Self {
        Error::Eh(error)
    }
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

    pub fn modify_reg(&mut self, reg: u16, mask: u32, set: u32) -> Result<(), I2C::Error> {
        let mut value = self.read_reg(reg)?;
        let old_v = value;
        value &= !mask;
        value |= set;
        println!("Modify: Reg {reg:04x} {old_v:08x} -> {value:08x}");
        self.write_reg(reg, value)
    }

    /// Mimic `setAsicRegBits`
    pub fn set_reg_bits(
        &mut self,
        reg: u16,
        mask: u32,
        value: u32,
    ) -> Result<(), Error<I2C::Error>> {
        let Some(shift) = first_bit(mask) else {
            return Err(Error::UserMask(mask));
        };

        let mut reg_value = self.read_reg(reg)?;

        reg_value &= !mask;
        reg_value |= value << shift;

        self.write_reg(reg, reg_value).map_err(Error::Eh)
    }

    /// Mimic `getAsicRegBits`
    pub fn get_reg_bits(&mut self, reg: u16, mask: u32) -> Result<u32, Error<I2C::Error>> {
        let Some(shift) = first_bit(mask) else {
            return Err(Error::UserMask(mask));
        };

        let reg_value = self.read_reg(reg)?;

        Ok((reg_value & mask) >> shift)
    }

    /// Mimic `setAsicRegBits`
    pub fn set_reg_bit(
        &mut self,
        reg: u16,
        offset: u32,
        value: bool,
    ) -> Result<(), Error<I2C::Error>> {
        if offset > 31 {
            return Err(Error::UserMask(offset));
        };

        let mut reg_value = self.read_reg(reg)?;

        if value {
            reg_value |= 1 << offset;
        } else {
            reg_value &= !(1 << offset);
        }

        self.write_reg(reg, reg_value).map_err(Error::Eh)
    }

    /// Mimic `getAsicRegBits`
    pub fn get_reg_bit(&mut self, reg: u16, offset: u32) -> Result<bool, Error<I2C::Error>> {
        if offset > 31 {
            return Err(Error::UserMask(offset));
        };

        let reg_value = self.read_reg(reg)?;

        Ok((reg_value >> offset) & 1 == 1)
    }

    pub fn get_i2c_gpio_pin_group(&mut self) -> Result<(u32, u32), Error<I2C::Error>> {
        let value = self.read_reg(Regs::I2cMst1Ctrl1.into())?;

        let sda = (value >> 10) & 0x07;
        let scl = (value >> 13) & 0x07;

        Ok((sda, scl))
    }

    pub fn get_chip_id(&mut self) -> Result<Chip, Error<I2C::Error>> {
        let value = self.read_reg(Regs::ChipID.into())?;
        Chip::try_from(value >> 8).map_err(Error::UnknownChipID)
    }

    pub fn get_soc_version(&mut self) -> Result<u16, Error<I2C::Error>> {
        let reg = Regs::ChipInfo.into();
        self.set_reg_bits(reg, 0xF << 16, 0xa)?;

        let mut reg = 0;

        for _ in 0..3 {
            reg <<= 4;
            let val = self.read_reg(reg)?;
            reg |= ((val >> 28) as u16) & 0xf;
        }

        self.set_reg_bits(reg, 0xF << 16, 0x0)?;

        Ok(reg)
    }

    pub fn phy_read_cl45(
        &mut self,
        phy_id: u8,
        dev_addr: u8,
        reg: u16,
    ) -> Result<u16, Error<I2C::Error>> {
        self.set_reg_bits(Regs::SmiAccessPhyCtrl3.into(), 0xFFFF, u32::from(phy_id))?;

        let tmp: u32 = (u32::from(dev_addr) << 19) | (u32::from(reg) << 3) | 0x3;

        self.write_reg(Regs::SmiAccessPhyCtrl1.into(), tmp)?;

        for _ in 0..100 {
            let val = self.read_reg(Regs::SmiAccessPhyCtrl1.into())?;
            // RTL8373_SMI_ACCESS_PHY_CTRL_1_CMD_OFFSET = 1 << 0
            // RTL8373_SMI_ACCESS_PHY_CTRL_1_FAIL_MASK = 7 << 24
            if val & (7 << 24 | 1) == 0 {
                // Success
                return self
                    .get_reg_bits(Regs::SmiAccessPhyCtrl2.into(), 0xFFFF)
                    .map(|val| val as u16);
            }
        }

        Err(Error::PhyAccessTimeOut)
    }

    pub fn phy_write_cl45(
        &mut self,
        phy_id: u8,
        dev_addr: u8,
        reg: u16,
        value: u16,
    ) -> Result<(), Error<I2C::Error>> {
        self.write_reg(Regs::SmiAccessPhyCtrl0.into(), 1 << u16::from(phy_id))?;

        self.set_reg_bits(Regs::SmiAccessPhyCtrl3.into(), 0xFFFF, u32::from(value))?;

        let tmp: u32 = (u32::from(dev_addr) << 19) | (u32::from(reg) << 3) | 0x7;

        self.write_reg(Regs::SmiAccessPhyCtrl1.into(), tmp)?;

        for _ in 0..100 {
            let val = self.read_reg(Regs::SmiAccessPhyCtrl1.into())?;
            // RTL8373_SMI_ACCESS_PHY_CTRL_1_CMD_OFFSET = 1 << 0
            // RTL8373_SMI_ACCESS_PHY_CTRL_1_FAIL_MASK = 7 << 24
            if val & (7 << 24 | 1) == 0 {
                // Success
                return Ok(());
            }
        }

        Err(Error::PhyAccessTimeOut)
    }

    pub fn rtl8224_reg_read(&mut self, reg: u16) -> Result<u32, Error<I2C::Error>> {
        // self.phy_write_cl45(0, 0x1e, reg, value )
        let lsb_val = self.phy_read_cl45(0, 0x1e, reg)?;
        let msb_val = self.phy_read_cl45(0, 0x1e, reg + 1)?;
        Ok(u32::from(msb_val) << 16 | u32::from(lsb_val))
    }

    pub fn rtl8224_reg_write(&mut self, reg: u16, value: u32) -> Result<(), Error<I2C::Error>> {
        // self.phy_write_cl45(0, 0x1e, reg, value )
        self.phy_write_cl45(0, 0x1e, reg, value as u16)?;
        self.phy_write_cl45(0, 0x1e, reg + 1, (value >> 16) as u16)
    }

    pub fn rtl8224_sds_reg_write(
        &mut self,
        sds_index: u8,
        sds_page: u8,
        sds_reg: u16,
        regdata: u32,
    ) -> Result<(), Error<I2C::Error>> {
        //   #define RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET                                                               (15)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_CMD_MASK                                                                 (0x1 << RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_RWOP_OFFSET                                                              (14)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_RWOP_MASK                                                                (0x1 << RTL8373_SDS_INDACS_CMD_SDS_RWOP_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_REGAD_OFFSET                                                             (7)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_REGAD_MASK                                                               (0x1F << RTL8373_SDS_INDACS_CMD_SDS_REGAD_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_PAGE_OFFSET                                                              (1)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_PAGE_MASK                                                                (0x3F << RTL8373_SDS_INDACS_CMD_SDS_PAGE_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_INDEX_OFFSET                                                             (0)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_INDEX_MASK                                                               (0x1 << RTL8373_SDS_INDACS_CMD_SDS_INDEX_OFFSET)
        let mut cnt = 100;
        loop {
            let val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;

            // RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET
            if val & (1 << 15) == 0 {
                // Success
                break;
            }

            if cnt == 0 {
                return Err(Error::PhyAccessTimeOut);
            }
            cnt -= 1;
        }

        // RTL8373_SDS_INDACS_WD_ADDR
        println!("WRITE: SdsIndacsWd: {regdata:04x}");
        self.rtl8224_reg_write(Regs::SdsIndacsWd.into(), regdata)?;

        // RTL8373_SDS_INDACS_CMD_SDS_INDEX_OFFSET
        let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        println!("WRITE: SdsIndacsCmd: {val:04x}");
        val = 0;
        if sds_index & 0x01 != 0 {
            val |= 1;
        } else {
            val &= !1;
        }
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_PAGE_MASK
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val &= !(0x3F << 1);
        val |= u32::from(sds_page & 0x3f) << 1;
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_REGAD_MASK
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val &= !(0x1F << 7);
        val |= u32::from(sds_reg & 0x1f) << 7;
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_RWOP_OFFSET
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val |= 1 << 14;
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val |= 1 << 15;
        println!("WRITE: SdsIndacsCmd: {val:04x}");
        self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        let mut cnt = 100;
        loop {
            // RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET
            let val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;

            if val & (1 << 15) == 0 {
                // Success
                break;
            }

            if cnt == 0 {
                return Err(Error::PhyAccessTimeOut);
            }
            cnt -= 1;
        }

        Ok(())
    }

    pub fn rtl8224_sds_reg_read(
        &mut self,
        sds_index: u8,
        sds_page: u8,
        sds_reg: u16,
    ) -> Result<u32, Error<I2C::Error>> {
        //   #define RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET                                                               (15)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_CMD_MASK                                                                 (0x1 << RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_RWOP_OFFSET                                                              (14)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_RWOP_MASK                                                                (0x1 << RTL8373_SDS_INDACS_CMD_SDS_RWOP_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_REGAD_OFFSET                                                             (7)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_REGAD_MASK                                                               (0x1F << RTL8373_SDS_INDACS_CMD_SDS_REGAD_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_PAGE_OFFSET                                                              (1)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_PAGE_MASK                                                                (0x3F << RTL8373_SDS_INDACS_CMD_SDS_PAGE_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_INDEX_OFFSET                                                             (0)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_INDEX_MASK
        //  (0x1 << RTL8373_SDS_INDACS_CMD_SDS_INDEX_OFFSET)
        let mut val;
        let mut cnt = 100;
        loop {
            val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;

            // RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET
            if val & (1 << 15) == 0 {
                println!("SdsIndacsCmd: {val:04x}");
                // Success
                break;
            }

            if cnt == 0 {
                return Err(Error::PhyAccessTimeOut);
            }
            cnt -= 1;
        }

        // RTL8373_SDS_INDACS_CMD_SDS_INDEX_OFFSET
        val = u32::from(sds_index & 0x01);
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_PAGE_MASK
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        // val &= !(0x3F << 1);
        val |= u32::from(sds_page & 0x3f) << 1;
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_REGAD_MASK
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        // val &= !(0x1F << 7);
        val |= u32::from(sds_reg & 0x1f) << 7;
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_RWOP_OFFSET
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val &= !(1 << 14);
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val |= 1 << 15;
        self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        let mut cnt = 100;
        loop {
            let val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;

            // RTL8373_SMI_ACCESS_PHY_CTRL_1_CMD_OFFSET = 1 << 0
            // RTL8373_SMI_ACCESS_PHY_CTRL_1_FAIL_MASK = 7 << 24
            if val & (1 << 15) == 0 {
                // Success
                break;
            }

            if cnt == 0 {
                return Err(Error::PhyAccessTimeOut);
            }
            cnt -= 1;
        }

        self.rtl8224_reg_read(Regs::SdsIndacsRd.into())
    }

    pub fn i2c_read(
        &mut self,
        scl: I2C_SCL,
        sda: I2C_SDA,
        dev: u8,
        addr: Addr,
        data: &mut [u8],
    ) -> Result<(), Error<I2C::Error>> {
        let (addr_data, addr_len) = match addr {
            Addr::None => (0x0, 0 << 20),
            Addr::One(v) => (u32::from(v), 1 << 20),
            Addr::Two(v) => (u32::from(u16::from_be_bytes(v)), 2 << 20),
            Addr::Three(v) => (
                u32::from(v[2]) << 16 | u32::from(v[1]) << 8 | u32::from(v[0]),
                3 << 20,
            ),
        };

        // println!("addr_data: {addr_data:08x}");
        self.write_reg(Regs::I2cAddrData.into(), addr_data)
            .map_err(|_err| Error::PhyAccessTimeOut)?;

        let Some(len) = u32::try_from(data.len())
            .ok()
            .map(|val| {
                if val == 0 || val > 16 {
                    None
                } else {
                    Some(val - 1)
                }
            })
            .map(|val| val.unwrap())
        else {
            return Err(Error::InvalidInput);
        };

        // #define RTL837X_REG_I2C_SCL_SHIFT 5
        // #define RTL837X_REG_I2C_SDA_SHIFT 2
        // 	.sds_settings[0].sds_settings_t.sfp.i2c = I2CBUS( GPIO41_I2C_SDA3_MDIO1, GPIO40_I2C_SCL3_MDC1 ), /* GPIO 40 */
        //  3 << 2 | 3 << 5
        // .sds_settings[1].sds_settings_t.sfp.i2c = I2CBUS( GPIO39_I2C_SDA4, GPIO40_I2C_SCL3_MDC1 )
        //  4 << 2 | 3 << 5

        let mut val = addr_len;
        val |= len << 16;
        val |= scl as u32;
        val |= sda as u32;
        val |= u32::from(dev) << 3;
        val |= 0x1;
        // println!("data: {val:08x}");

        // Rust 1 0001  0111 0010  1011 0001
        // C    1 0010  0111 0010  1011 0001

        // data = 0x001f6e89;
        self.write_reg(Regs::I2cMst1Ctrl1.into(), val)
            .map_err(|err| Error::PhyAccessTimeOut)?;

        let mut value: u32;
        loop {
            value = self
                .read_reg(Regs::I2cMst1Ctrl1.into())
                .map_err(|err| Error::PhyAccessTimeOut)?;
            // println!("value: {value:08x}");
            if value & 0x1 == 0 {
                break;
            }
        }
        if value & 0x2 != 0 {
            return Err(Error::I2CNack);
        }

        for (idx, d) in data.chunks_mut(4).enumerate() {
            value = self
                .read_reg(Regs::I2cData as u16 + (idx * 4) as u16)
                .map_err(|_err| Error::PhyAccessTimeOut)?;
            let len = d.len();
            // println!("idx: {idx}, value = {value:08x}, d = {len}");

            d[0..len].copy_from_slice(&value.to_le_bytes()[0..len]);
        }

        Ok(())
    }

    pub fn i2c_write(
        &mut self,
        scl: I2C_SCL,
        sda: I2C_SDA,
        dev: u8,
        addr: Addr,
        data: &[u8],
    ) -> Result<(), Error<I2C::Error>> {
        let (addr_data, addr_len) = match addr {
            Addr::None => (0x0, 0 << 20),
            Addr::One(v) => (u32::from(v), 1 << 20),
            Addr::Two(v) => (u32::from(u16::from_be_bytes(v)), 2 << 20),
            Addr::Three(v) => (
                u32::from(v[2]) << 16 | u32::from(v[1]) << 8 | u32::from(v[0]),
                3 << 20,
            ),
        };

        // println!("addr_data: {addr_data:08x}");
        self.write_reg(Regs::I2cAddrData.into(), addr_data)
            .map_err(|_err| Error::PhyAccessTimeOut)?;

        let Some(len) = u32::try_from(data.len())
            .ok()
            .map(|val| {
                if val == 0 || val > 16 {
                    None
                } else {
                    Some(val - 1)
                }
            })
            .map(|val| val.unwrap())
        else {
            return Err(Error::InvalidInput);
        };

        let mut value = [0; 4];
        for (idx, d) in data.chunks(4).enumerate() {
            let len = d.len();
            value[0..len].copy_from_slice(d);

            self.write_reg(
                Regs::I2cData as u16 + (idx * 4) as u16,
                u32::from_le_bytes(value),
            )
            .map_err(|_err| Error::PhyAccessTimeOut)?;
            // println!("idx: {idx}, value = {value:08x}, d = {len}");
        }

        // #define RTL837X_REG_I2C_SCL_SHIFT 5
        // #define RTL837X_REG_I2C_SDA_SHIFT 2
        // 	.sds_settings[0].sds_settings_t.sfp.i2c = I2CBUS( GPIO41_I2C_SDA3_MDIO1, GPIO40_I2C_SCL3_MDC1 ), /* GPIO 40 */
        //  3 << 2 | 3 << 5
        // .sds_settings[1].sds_settings_t.sfp.i2c = I2CBUS( GPIO39_I2C_SDA4, GPIO40_I2C_SCL3_MDC1 )
        //  4 << 2 | 3 << 5

        let mut val = addr_len;
        val |= len << 16;
        val |= scl as u32;
        val |= sda as u32;
        val |= u32::from(dev) << 3;
        val |= 0x4 | 0x1;
        // println!("data: {val:08x}");

        // Rust 1 0001  0111 0010  1011 0001
        // C    1 0010  0111 0010  1011 0001

        // data = 0x001f6e89;
        self.write_reg(Regs::I2cMst1Ctrl1.into(), val)
            .map_err(|err| Error::PhyAccessTimeOut)?;

        let mut value: u32;
        loop {
            value = self
                .read_reg(Regs::I2cMst1Ctrl1.into())
                .map_err(|err| Error::PhyAccessTimeOut)?;
            // println!("value: {value:08x}");
            if value & 0x1 == 0 {
                break;
            }
        }
        if value & 0x2 != 0 {
            return Err(Error::I2CNack);
        }

        Ok(())
    }

    pub fn sds_reg_modify(
        &mut self,
        sds_index: u8,
        sds_page: u8,
        sds_regad: u8,
        mask: u16,
        set: u16,
    ) -> Result<(), Error<I2C::Error>> {
        let mut val = self.sds_reg_read(sds_index, sds_page, sds_regad)?;
        val &= !(mask);
        val |= set;
        self.sds_reg_write(sds_index, sds_page, sds_regad, val)
    }

    pub fn sds_reg_write(
        &mut self,
        sds_index: u8,
        sds_page: u8,
        sds_regad: u8,
        regdata: u16,
    ) -> Result<(), Error<I2C::Error>> {
        //   #define RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET                                                               (15)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_CMD_MASK                                                                 (0x1 << RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_RWOP_OFFSET                                                              (14)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_RWOP_MASK                                                                (0x1 << RTL8373_SDS_INDACS_CMD_SDS_RWOP_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_REGAD_OFFSET                                                             (7)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_REGAD_MASK                                                               (0x1F << RTL8373_SDS_INDACS_CMD_SDS_REGAD_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_PAGE_OFFSET                                                              (1)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_PAGE_MASK                                                                (0x3F << RTL8373_SDS_INDACS_CMD_SDS_PAGE_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_INDEX_OFFSET                                                             (0)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_INDEX_MASK                                                               (0x1 << RTL8373_SDS_INDACS_CMD_SDS_INDEX_OFFSET)
        let mut cnt = 100;
        loop {
            let val = self.read_reg(Regs::SdsIndacsCmd.into())?;

            // RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET
            if val & (1 << 15) == 0 {
                // Success
                break;
            }

            if cnt == 0 {
                return Err(Error::PhyAccessTimeOut);
            }
            cnt -= 1;
        }

        // RTL8373_SDS_INDACS_WD_ADDR
        println!("WRITE: SdsIndacsWd: {regdata:04x}");
        self.write_reg(Regs::SdsIndacsWd.into(), u32::from(regdata))?;

        // RTL8373_SDS_INDACS_CMD_SDS_INDEX_OFFSET
        let mut val = self.read_reg(Regs::SdsIndacsCmd.into())?;
        println!("WRITE: SdsIndacsCmd: {val:04x}");
        val = 0;
        if sds_index & 0x01 != 0 {
            val |= 1;
        } else {
            val &= !1;
        }
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_PAGE_MASK
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val &= !(0x3F << 1);
        val |= u32::from(sds_page & 0x3f) << 1;
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_REGAD_MASK
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val &= !(0x1F << 7);
        val |= u32::from(sds_regad & 0x1f) << 7;
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_RWOP_OFFSET
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val |= 1 << 14;
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val |= 1 << 15;
        println!("WRITE: SdsIndacsCmd: {val:04x}");
        self.write_reg(Regs::SdsIndacsCmd.into(), val)?;

        let mut cnt = 100;
        loop {
            // RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET
            let val = self.read_reg(Regs::SdsIndacsCmd.into())?;

            if val & (1 << 15) == 0 {
                // Success
                break;
            }

            if cnt == 0 {
                return Err(Error::PhyAccessTimeOut);
            }
            cnt -= 1;
        }

        Ok(())
    }
    pub fn sds_reg_read(
        &mut self,
        sds_index: u8,
        sds_page: u8,
        sds_regad: u8,
    ) -> Result<u16, Error<I2C::Error>> {
        //   #define RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET                                                               (15)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_CMD_MASK                                                                 (0x1 << RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_RWOP_OFFSET                                                              (14)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_RWOP_MASK                                                                (0x1 << RTL8373_SDS_INDACS_CMD_SDS_RWOP_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_REGAD_OFFSET                                                             (7)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_REGAD_MASK                                                               (0x1F << RTL8373_SDS_INDACS_CMD_SDS_REGAD_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_PAGE_OFFSET                                                              (1)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_PAGE_MASK                                                                (0x3F << RTL8373_SDS_INDACS_CMD_SDS_PAGE_OFFSET)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_INDEX_OFFSET                                                             (0)
        //   #define RTL8373_SDS_INDACS_CMD_SDS_INDEX_MASK                                                               (0x1 << RTL8373_SDS_INDACS_CMD_SDS_INDEX_OFFSET)
        let mut cnt = 100;
        loop {
            let val = self.read_reg(Regs::SdsIndacsCmd.into())?;

            // RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET
            if val & (1 << 15) == 0 {
                // Success
                break;
            }

            if cnt == 0 {
                return Err(Error::PhyAccessTimeOut);
            }
            cnt -= 1;
        }

        // RTL8373_SDS_INDACS_CMD_SDS_INDEX_OFFSET
        let mut val = self.read_reg(Regs::SdsIndacsCmd.into())?;
        println!("WRITE: SdsIndacsCmd: {val:04x}");
        val = 0;
        if sds_index & 0x01 != 0 {
            val |= 1;
        } else {
            val &= !1;
        }
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_PAGE_MASK
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val &= !(0x3F << 1);
        val |= u32::from(sds_page & 0x3f) << 1;
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_REGAD_MASK
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val &= !(0x1F << 7);
        val |= u32::from(sds_regad & 0x1f) << 7;
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_RWOP_OFFSET
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val &= !(1 << 14);
        // self.rtl8224_reg_write(Regs::SdsIndacsCmd.into(), val)?;

        // RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET
        // let mut val = self.rtl8224_reg_read(Regs::SdsIndacsCmd.into())?;
        val |= 1 << 15;
        println!("WRITE: SdsIndacsCmd: {val:04x}");
        self.write_reg(Regs::SdsIndacsCmd.into(), val)?;

        let mut cnt = 100;
        loop {
            // RTL8373_SDS_INDACS_CMD_SDS_CMD_OFFSET
            let val = self.read_reg(Regs::SdsIndacsCmd.into())?;

            if val & (1 << 15) == 0 {
                // Success
                break;
            }

            if cnt == 0 {
                return Err(Error::PhyAccessTimeOut);
            }
            cnt -= 1;
        }

        // RTL8373_SDS_INDACS_WD_ADDR
        let val = self.read_reg(Regs::SdsIndacsRd.into())?;

        Ok(val as u16)
    }
}

pub enum Addr {
    None,
    One(u8),
    Two([u8; 2]),
    Three([u8; 3]),
}

#[derive(Debug, Clone, Copy)]
pub enum Chip {
    Rtl8372,
    Rtl8373,
    Rtl8372N,
    Rtl8373N,
    Rtl8224,
    Rtl8224N,
    RTL8366U,
}

impl TryFrom<u32> for Chip {
    type Error = u32;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0x837300 => Ok(Chip::Rtl8373),
            0x837370 => Ok(Chip::Rtl8373N),
            0x837200 => Ok(Chip::Rtl8372),
            0x837270 => Ok(Chip::Rtl8372N),
            0x822400 => Ok(Chip::Rtl8224),
            0x822470 => Ok(Chip::Rtl8224N),
            0x8366A8 => Ok(Chip::RTL8366U),
            _ => Err(value),
        }
    }
}

fn first_bit(mask: u32) -> Option<u32> {
    let bit = mask.trailing_zeros();
    if bit < 32 { Some(bit) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::i2c::{Mock as I2cMock, Transaction as I2cTransaction};

    #[test]
    fn shift_test() {
        assert_eq!(first_bit(0x01), Some(0));
        assert_eq!(first_bit(0x02), Some(1));
        assert_eq!(first_bit(0x8000_0000), Some(31));
        assert_eq!(first_bit(0x00), None);
    }

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
