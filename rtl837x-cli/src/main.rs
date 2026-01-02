const SLAVE_ADDR: u8 = 0x5c;
use rtl837x::Regs;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let path = "/dev/i2c-7";
    let mut bus = match linux_embedded_hal::I2cdev::new(path) {
        Ok(bus) => bus,
        Err(e) => {
            println!("Error opening I2C Bus {} {:?}", path, e);
            return Err(e.into());
        }
    };

    let mut rtldev = rtl837x::Rtl837x::new(&mut bus, SLAVE_ADDR)?;

    let chip_id = rtldev.read_reg(Regs::ChipID);

    println!("Chip ID: {:08x?}", chip_id);
    Ok(())
}
