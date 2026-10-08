const SLAVE_ADDR: u8 = 0x5c;
use std::{thread::sleep, time::Duration};

use embedded_hal::i2c::I2c;
use rtl837x::{Error, I2C_SCL, I2C_SDA, Regs, Rtl837x, phy_modify, phy_read, phy_write};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, MouseButton,
        MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use serde::{Deserialize, Serialize};
use serde_json::from_reader;
use std::fs::File;
use std::io::{self, Write};

#[derive(Serialize, Deserialize, Debug)]
struct Register {
    name: String,
    address: u16,
    value: u32,
}

struct App {
    registers: Vec<Register>,
    selected: ListState,
}

impl App {
    fn new(registers: Vec<Register>) -> Self {
        Self {
            registers,
            selected: ListState::default(),
        }
    }

    fn toggle_bit(&mut self, bit: usize) {
        if let Some(register) = self
            .registers
            .get_mut(self.selected.selected().unwrap_or(0))
        {
            register.value ^= 1 << bit; // Toggle the specified bit
        }
    }

    fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            terminal.draw(|f| {
                let size = f.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([Constraint::Percentage(100)].as_ref())
                    .split(size);

                let items: Vec<ListItem> = self
                    .registers
                    .iter()
                    .map(|r| {
                        ListItem::new(format!(
                            "{}: {:04x} = {:08x} {:032b}",
                            r.name, r.address, r.value, r.value
                        ))
                    })
                    .collect();

                let list = List::new(items)
                    .block(Block::default().title("Registers").borders(Borders::ALL))
                    .highlight_style(
                        ratatui::style::Style::default().bg(ratatui::style::Color::Yellow),
                    );

                f.render_stateful_widget(list, chunks[0], &mut self.selected); // use &mut self.selected
            })?;

            if event::poll(std::time::Duration::from_millis(10))? {
                if let Event::Mouse(mouse_event) = event::read()? {
                    if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                        let bit = (mouse_event.column as usize) % 32; // Assuming a fixed width layout for bits
                        self.toggle_bit(bit);
                    }
                } else if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                    match code {
                        KeyCode::Up => {
                            self.selected.select_next();
                        }
                        KeyCode::Down => {
                            self.selected.select_previous();
                        }
                        KeyCode::Esc => break,
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
}

// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let file = File::open("registers.json")?;
//     let registers: Vec<Register> = from_reader(file)?;

//     let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
//     execute!(terminal.backend_mut(), EnterAlternateScreen)?;
//     execute!(std::io::stdout(), EnableMouseCapture)?;
//     terminal.clear()?;

//     let mut app = App::new(registers);

//     app.run(&mut terminal)?;

//     execute!(std::io::stdout(), DisableMouseCapture)?;
//     execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
//     terminal.show_cursor()?;

//     Ok(())
// }

struct Reg {
    name: &'static str,
    addr: u16,
}

impl Reg {
    fn new(name: &'static str, addr: u16) -> Self {
        Self { name, addr }
    }
}

fn marvell_config_aneg<'bus, I2C: I2c>(
    dev: &mut Rtl837x<'bus, I2C>,
) -> Result<(), Error<I2C::Error>> {
    genphy_restart_aneg(dev)?;

    genphy_soft_reset(dev)
}

const BMCR_ANRESTART: u16 = 0x200;
const BMCR_ISOLATE: u16 = 0x400;
const BMCR_ANENABLE: u16 = 0x1000;
const BMCR_RESET: u16 = 0x8000;

const MII_MARVELL_PHY_PAGE: u8 = 22;

fn genphy_soft_reset<'bus, I2C: I2c>(
    dev: &mut Rtl837x<'bus, I2C>,
) -> Result<(), Error<I2C::Error>> {
    phy_modify(dev, 0x00, BMCR_ISOLATE, BMCR_RESET | BMCR_ANRESTART)
}

fn genphy_restart_aneg<'bus, I2C: I2c>(
    dev: &mut Rtl837x<'bus, I2C>,
) -> Result<(), Error<I2C::Error>> {
    let reg = 0x00;
    phy_write(dev, reg, 0x00)?;
    let mut val = phy_read(dev, reg)?;
    println!("Reg: {reg:02x} = {:04x}", val);
    if (val & BMCR_ANENABLE) == 0 {
        val &= !BMCR_ISOLATE;
        val |= BMCR_ANRESTART | BMCR_ANENABLE;
        println!("Write {val:04x}");
        phy_write(dev, reg, val)?;
    }

    Ok(())
}

const AN_1P25G_CHIPA: [(u8, u8, u16); 17] = [
    (0x21, 0x10, 0x6480),
    (0x21, 0x13, 0x0400),
    (0x21, 0x18, 0x6d02),
    (0x21, 0x1b, 0x424e),
    (0x21, 0x1d, 0x0002),
    (0x36, 0x1c, 0x1390),
    (0x36, 0x14, 0x003F),
    (0x36, 0x10, 0x0300),
    (0x24, 0x04, 0x0080),
    (0x24, 0x7, 0x1201),
    (0x24, 0x09, 0x0601),
    (0x24, 0x0b, 0x232c),
    (0x24, 0x0c, 0x9217),
    (0x24, 0x0f, 0x5B50),
    (0x24, 0x15, 0xe7c1),
    (0x24, 0x16, 0x0443),
    (0x24, 0x1d, 0xabb0),
];

const DIG_PATCH_MAC: [(u8, u8, u16); 8] = [
    (6, 18, 0x5078),
    (7, 6, 0x9401),
    (7, 8, 0x9401),
    (7, 10, 0x9401),
    (7, 12, 0x9401),
    (31, 11, 0x0003),
    (6, 3, 0xc45c),
    (6, 31, 0x2100),
];

const DIG_PATCH_PHY: [(u8, u8, u16); 4] = [
    (6, 18, 0x5078),
    (6, 3, 0xc45c),
    (6, 30, 0x000C),
    (6, 31, 0x2100),
];

fn fw_reset_flow_tgx<'bus, I2C: I2c>(
    dev: &mut Rtl837x<'bus, I2C>,
    sds: u8,
) -> Result<(), Error<I2C::Error>> {
    let val = dev.sds_reg_read(sds, 0x20, 0)?;
    if (val >> 4) & 0x03 != 0x01 {
        let val = dev.sds_reg_read(sds, 1, 0x1d)?;
        let sig_ok = (val >> 8) & 1 == 1;
        let sync_ok = val & 1 == 1;
        let link_ok = (val >> 4) & 1 == 1;
        println!("SIG {sig_ok}, Sync {sync_ok}, Link {link_ok}");
        if sig_ok {
            let val = dev.sds_reg_read(sds, 0, 0)? & !(1 << 1);
            if sync_ok || !link_ok {
                dev.sds_reg_write(sds, 0, 0, val | 0x02)?;
                dev.sds_reg_write(sds, 0, 0, val)?;
                dev.sds_reg_write(sds, 0, 0, val | 0x02)?;
            }
        }
    }

    Ok(())
}

pub fn __ffs(val: u32) -> u32 {
    val.trailing_zeros()
}

fn sds_nway_set<'bus, I2C: I2c>(
    dev: &mut Rtl837x<'bus, I2C>,
    sds: u8,
    an_en: bool,
) -> Result<(), Error<I2C::Error>> {
    // dal_rtl8373_sds_regbits_write(SDS_INDX, 0, 2, 0x3<<8, 0x3);
    // dal_rtl8373_sds_regbits_write(SDS_INDX, 0, 4, 0x1<<2, 0x1);

    let mut val = dev.sds_reg_read(sds, 0, 2)?;
    val &= !(0x3 << 8);
    if an_en {
        val |= 0x3 << 8;
    } else {
        val |= 0x1 << 8;
    }
    dev.sds_reg_write(sds, 0, 2, val)?;

    let mut val = dev.sds_reg_read(sds, 0, 4)?;
    val &= !(1 << 2);
    val |= 1 << 2;
    dev.sds_reg_write(sds, 0, 4, val)
}

const SDS_PAGE_FRC: u8 = 0x20;
const SDS_REG_FRC: u8 = 0x00;

const SDS_FRC_RX_EN_VAL_MASK: u16 = 1 << 5;
const SDS_FRC_RX_EN_ON_MASK: u16 = 1 << 4;

fn serdes_off<'bus, I2C: I2c>(
    rtldev: &mut Rtl837x<'bus, I2C>,
    sds: u8,
) -> Result<(), Error<I2C::Error>> {
    rtldev.sds_reg_modify(sds, SDS_PAGE_FRC, SDS_REG_FRC, 3 << 4, 3 << 4)?;
    sleep(Duration::from_millis(20));
    rtldev.sds_reg_modify(sds, SDS_PAGE_FRC, SDS_REG_FRC, 3 << 4, 1 << 4)?;
    sleep(Duration::from_millis(50));

    rtldev.sds_reg_modify(sds, SDS_PAGE_FRC, SDS_REG_FRC, 3 << 6, 1 << 6)?;
    sleep(Duration::from_millis(20));
    rtldev.sds_reg_modify(sds, SDS_PAGE_FRC, SDS_REG_FRC, 3 << 6, 3 << 6)?;
    sleep(Duration::from_millis(50));

    rtldev.sds_reg_modify(sds, SDS_PAGE_FRC, SDS_REG_FRC, 3 << 10, 3 << 10)?;
    sleep(Duration::from_millis(20));
    rtldev.sds_reg_modify(sds, SDS_PAGE_FRC, SDS_REG_FRC, 3 << 10, 1 << 10)?;
    sleep(Duration::from_millis(50));

    Ok(())
}

fn serdes_on<'bus, I2C: I2c>(
    rtldev: &mut Rtl837x<'bus, I2C>,
    sds: u8,
) -> Result<(), Error<I2C::Error>> {
    rtldev.sds_reg_modify(sds, SDS_PAGE_FRC, SDS_REG_FRC, 3 << 10, 1 << 10)?;
    sleep(Duration::from_millis(20));
    rtldev.sds_reg_modify(sds, SDS_PAGE_FRC, SDS_REG_FRC, 3 << 10, 3 << 10)?;
    sleep(Duration::from_millis(50));
    rtldev.sds_reg_modify(sds, SDS_PAGE_FRC, SDS_REG_FRC, 3 << 10, 0 << 10)?;
    sleep(Duration::from_millis(20));

    rtldev.sds_reg_modify(sds, SDS_PAGE_FRC, SDS_REG_FRC, 3 << 6, 3 << 6)?;
    sleep(Duration::from_millis(20));
    rtldev.sds_reg_modify(sds, SDS_PAGE_FRC, SDS_REG_FRC, 3 << 6, 1 << 6)?;
    sleep(Duration::from_millis(50));
    rtldev.sds_reg_modify(sds, SDS_PAGE_FRC, SDS_REG_FRC, 3 << 6, 0 << 6)?;
    sleep(Duration::from_millis(20));

    rtldev.sds_reg_modify(
        sds,
        SDS_PAGE_FRC,
        SDS_REG_FRC,
        SDS_FRC_RX_EN_VAL_MASK | SDS_FRC_RX_EN_ON_MASK,
        3 << 4,
    )?;
    sleep(Duration::from_millis(20));
    rtldev.sds_reg_modify(
        sds,
        SDS_PAGE_FRC,
        SDS_REG_FRC,
        SDS_FRC_RX_EN_VAL_MASK | SDS_FRC_RX_EN_ON_MASK,
        1 << 4,
    )?;
    sleep(Duration::from_millis(50));
    rtldev.sds_reg_modify(
        sds,
        SDS_PAGE_FRC,
        SDS_REG_FRC,
        SDS_FRC_RX_EN_VAL_MASK | SDS_FRC_RX_EN_ON_MASK,
        0 << 4,
    )?;
    sleep(Duration::from_millis(50));

    Ok(())
}

const RTL8373_SDS_MODE_SEL_CFG_MAC8_8221B_OFFSET: u32 = 22;
const RTL8373_SDS_MODE_SEL_CFG_MAC8_8221B_MASK: u32 =
    0x1 << RTL8373_SDS_MODE_SEL_CFG_MAC8_8221B_OFFSET;
const RTL8373_SDS_MODE_SEL_CFG_MAC3_8221B_OFFSET: u32 = 21;
const RTL8373_SDS_MODE_SEL_CFG_MAC3_8221B_MASK: u32 =
    0x1 << RTL8373_SDS_MODE_SEL_CFG_MAC3_8221B_OFFSET;
const RTL8373_SDS_MODE_SEL_SDS1_USX_SUB_MODE_OFFSET: u32 = 16;
const RTL8373_SDS_MODE_SEL_SDS1_USX_SUB_MODE_MASK: u32 =
    0x1F << RTL8373_SDS_MODE_SEL_SDS1_USX_SUB_MODE_OFFSET;
const RTL8373_SDS_MODE_SEL_SDS0_USX_SUB_MODE_OFFSET: u32 = 10;
const RTL8373_SDS_MODE_SEL_SDS0_USX_SUB_MODE_MASK: u32 =
    0x1F << RTL8373_SDS_MODE_SEL_SDS0_USX_SUB_MODE_OFFSET;
const RTL8373_SDS_MODE_SEL_SDS1_MODE_SEL_OFFSET: u32 = 5;
const RTL8373_SDS_MODE_SEL_SDS1_MODE_SEL_MASK: u32 =
    0x1F << RTL8373_SDS_MODE_SEL_SDS1_MODE_SEL_OFFSET;
const RTL8373_SDS_MODE_SEL_SDS0_MODE_SEL_OFFSET: u32 = 0;
const RTL8373_SDS_MODE_SEL_SDS0_MODE_SEL_MASK: u32 =
    0x1F << RTL8373_SDS_MODE_SEL_SDS0_MODE_SEL_OFFSET;

#[derive(Debug, Clone, Copy)]
pub enum PhyInterfaceT {
    PHY_INTERFACE_MODE_USXGMII = 0x0d,
    PHY_INTERFACE_MODE_10GBASER = 0x1a,
    PHY_INTERFACE_MODE_2500BASEX = 0x16,
    PHY_INTERFACE_MODE_1000BASEX = 0x02,
    PHY_INTERFACE_MODE_SGMII = 0x04,
    PHY_INTERFACE_MODE_100BASEX = 0x05,
}

const SDS_PAGE_CTRL00: u8 = 0x00;
const SDS_REG_CTRL00_REG00: u8 = 0x00;
const SDS_REG_CTRL00_REG02: u8 = 0x02;
const SDS_REG_CTRL00_REG04: u8 = 0x04;

const SDS_PAGE_CTRL01: u8 = 0x01;
const SDS_REG_CTRL01_XSG_STS: u8 = 0x02;

const SDS_CTRL01_XSG_STS_SIG_OK: u16 = 1 << 8;
const SDS_CTRL01_XSG_STS_LINK_OK: u16 = 1 << 4;
const SDS_CTRL01_XSG_STS_SYNC_OK: u16 = 1 << 0;

fn sds_reset_x<'bus, I2C: I2c>(
    dev: &mut Rtl837x<'bus, I2C>,
    sds: u8,
) -> Result<(), Error<I2C::Error>> {
    let val = dev.sds_reg_read(sds, SDS_PAGE_FRC, SDS_REG_FRC)?;

    if (val & (SDS_FRC_RX_EN_ON_MASK | SDS_FRC_RX_EN_VAL_MASK)) >> 4 != 0x01 {
        let val = dev.sds_reg_read(sds, SDS_PAGE_CTRL01, SDS_REG_CTRL01_XSG_STS)?;
        let sig_ok = (val & SDS_CTRL01_XSG_STS_SIG_OK) != 0;
        let sync_ok = (val & SDS_CTRL01_XSG_STS_SYNC_OK) != 0;
        let link_ok = (val & SDS_CTRL01_XSG_STS_LINK_OK) != 0;
        println!("SIG {sig_ok}, Sync {sync_ok}, Link {link_ok}");
        if sig_ok {
            let bit = 1 << 1;
            let val = dev.sds_reg_read(sds, SDS_PAGE_CTRL00, SDS_REG_CTRL00_REG00)? & !(bit);
            if sync_ok || !link_ok {
                println!("Reseting SDS");
                dev.sds_reg_write(sds, SDS_PAGE_CTRL00, SDS_REG_CTRL00_REG00, val | bit)?;
                sleep(Duration::from_millis(20));
                dev.sds_reg_write(sds, SDS_PAGE_CTRL00, SDS_REG_CTRL00_REG00, val)?;
                sleep(Duration::from_millis(20));
                dev.sds_reg_write(sds, SDS_PAGE_CTRL00, SDS_REG_CTRL00_REG00, val | bit)?;
                sleep(Duration::from_millis(20));
            }
        }
    }

    Ok(())
}

fn serdes_an_patch<'bus, I2C: I2c>(
    dev: &mut Rtl837x<'bus, I2C>,
    sds: u8,
    interface: PhyInterfaceT,
) -> Result<(), Error<I2C::Error>> {
    println!("serdes_an_patch");
    let mut patch: Option<&[(u8, u8, u16)]> = None;
    match interface {
        PhyInterfaceT::PHY_INTERFACE_MODE_USXGMII => todo!(),
        PhyInterfaceT::PHY_INTERFACE_MODE_10GBASER => todo!(),
        PhyInterfaceT::PHY_INTERFACE_MODE_2500BASEX => todo!(),
        PhyInterfaceT::PHY_INTERFACE_MODE_1000BASEX | PhyInterfaceT::PHY_INTERFACE_MODE_SGMII => {
            patch = Some(&DIG_PATCH_MAC);
        }
        PhyInterfaceT::PHY_INTERFACE_MODE_100BASEX => todo!(),
    }

    if let Some(patch) = patch {
        for (sds_page, sds_reg, sds_value) in patch {
            dev.sds_reg_write(sds, *sds_page, *sds_reg, *sds_value)?;
        }
    }

    Ok(())
}

fn serdes_mac_patch<'bus, I2C: I2c>(
    dev: &mut Rtl837x<'bus, I2C>,
    sds: u8,
) -> Result<(), Error<I2C::Error>> {
    println!("Patch MAC");
    for (sds_page, sds_reg, sds_value) in DIG_PATCH_MAC {
        dev.sds_reg_write(sds, sds_page, sds_reg, sds_value)?;
    }
    Ok(())
}

const SDS_PAGE_CTRL02: u8 = 0x02;
const SDS_REG_CTRL02_XSG_AN: u8 = 0x04;

fn sds_pcs_config<'bus, I2C: I2c>(
    dev: &mut Rtl837x<'bus, I2C>,
    sds: u8,
    interface: PhyInterfaceT,
    en_inband: bool,
    permit_pause_to_mac: bool,
) -> Result<(), Error<I2C::Error>> {
    println!("Change SDS {sds} to {interface:?}");

    let mask = if sds == 0 {
        RTL8373_SDS_MODE_SEL_CFG_MAC3_8221B_MASK
    } else {
        RTL8373_SDS_MODE_SEL_CFG_MAC8_8221B_MASK
    };
    dev.modify_reg(Regs::SdsModeSel.into(), mask, 0)?;

    serdes_off(dev, sds)?;

    let mask = if sds == 0 {
        RTL8373_SDS_MODE_SEL_SDS0_USX_SUB_MODE_MASK
    } else {
        RTL8373_SDS_MODE_SEL_SDS1_USX_SUB_MODE_MASK
    };
    dev.modify_reg(Regs::SdsModeSel.into(), mask, 0)?;

    let mask = if sds == 0 {
        RTL8373_SDS_MODE_SEL_SDS0_MODE_SEL_MASK
    } else {
        RTL8373_SDS_MODE_SEL_SDS1_MODE_SEL_MASK
    };
    let mut value = interface as u32;
    if sds == 1 {
        value <<= 5;
    }
    dev.modify_reg(Regs::SdsModeSel.into(), mask, value)?;

    // apply patch
    serdes_an_patch(dev, sds, interface)?;
    serdes_mac_patch(dev, sds)?;

    sleep(Duration::from_millis(250));

    // Set Auto Negotiation Pause/AsymPause
    match interface {
        PhyInterfaceT::PHY_INTERFACE_MODE_10GBASER | PhyInterfaceT::PHY_INTERFACE_MODE_100BASEX => {
            todo!()
        }
        PhyInterfaceT::PHY_INTERFACE_MODE_2500BASEX
        | PhyInterfaceT::PHY_INTERFACE_MODE_1000BASEX => {
            dev.sds_reg_modify(sds, 0x1F, 5, 1 << 2, 1 << 2)?;
            dev.sds_reg_modify(sds, 0x1F, 5, 1 << 3, 0)?;
            dev.sds_reg_modify(sds, SDS_PAGE_CTRL02, SDS_REG_CTRL02_XSG_AN, 3, 3)?;
        }
        _ => (),
    }

    // Auto Negotiation
    match interface {
        PhyInterfaceT::PHY_INTERFACE_MODE_SGMII
        | PhyInterfaceT::PHY_INTERFACE_MODE_1000BASEX
        | PhyInterfaceT::PHY_INTERFACE_MODE_2500BASEX => {
            dev.sds_reg_modify(
                sds,
                SDS_PAGE_CTRL00,
                SDS_REG_CTRL00_REG02,
                3 << 8,
                if en_inband { 3 } else { 1 } << 8,
            )?;

            /* set SP_CFG_EN_LINK_FIB1G for enable fiberNwayForceLink */
            dev.sds_reg_modify(sds, SDS_PAGE_CTRL00, SDS_REG_CTRL00_REG04, 1 << 2, 1 << 2)?;
        }
        PhyInterfaceT::PHY_INTERFACE_MODE_USXGMII
        | PhyInterfaceT::PHY_INTERFACE_MODE_10GBASER
        | PhyInterfaceT::PHY_INTERFACE_MODE_100BASEX => {
            todo!()
        }
    }

    serdes_on(dev, sds)?;

    dev.sds_reg_modify(sds, 0x1f, 0x00, 0xFFFF, 0xb)?;

    sleep(Duration::from_millis(50));

    dev.sds_reg_modify(sds, 0x1f, 0x00, 0xFFFF, 0x0)?;

    sleep(Duration::from_millis(50));

    match interface {
        PhyInterfaceT::PHY_INTERFACE_MODE_USXGMII | PhyInterfaceT::PHY_INTERFACE_MODE_10GBASER => {
            todo!()
        }
        PhyInterfaceT::PHY_INTERFACE_MODE_2500BASEX
        | PhyInterfaceT::PHY_INTERFACE_MODE_1000BASEX
        | PhyInterfaceT::PHY_INTERFACE_MODE_SGMII
        | PhyInterfaceT::PHY_INTERFACE_MODE_100BASEX => sds_reset_x(dev, sds)?,
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "/dev/i2c-7";
    let mut bus = match linux_embedded_hal::I2cdev::new(path) {
        Ok(bus) => bus,
        Err(e) => {
            println!("Error opening I2C Bus {} {:?}", path, e);
            return Err(e.into());
        }
    };

    let mut rtldev = Rtl837x::new(&mut bus, SLAVE_ADDR);

    let soc = rtldev.get_chip_id()?;
    println!("SOC: {soc:?}");

    let soc = rtldev.get_soc_version()?;
    println!("SOC_REVISION: {soc:03x}");

    let mut data = [0; 16];

    for reg in (0..96).into_iter().step_by(16) {
        rtldev.i2c_read(
            I2C_SCL::GPIO40_SCL3_MDC1,
            I2C_SDA::GPIO41_SDA3_MDIO1,
            0x50,
            rtl837x::Addr::One(reg),
            &mut data,
        )?;

        let mut ascii = String::new();
        for (idx, &b) in data.iter().enumerate() {
            print!("{b:02x} ");

            let c: char = if b.is_ascii_control() {
                '.'
            } else {
                char::from_u32(u32::from(b)).unwrap_or('.')
            };
            ascii.push(c);

            if idx % 16 == 15 {
                println!(" -- `{ascii}`");
                ascii.clear();
            }
        }
    }
    let r = [0x11ff, 0x01ff, 0x01ff, 0x0cc2, 0x0c01];

    let mut cnt = 0;

    for reg in 2..4 {
        let val = phy_read(&mut rtldev, reg)?;

        println!("Reg: {reg:02x} = {:04x}", val);
        if r[usize::from(reg)] != val {
            eprintln!("Error!");
            break;
        }
        if cnt % 25 == 0 {
            println!("Cnt {cnt}");
        }
        cnt += 1;
    }

    // https://elixir.bootlin.com/linux/v7.2.9/source/drivers/net/phy/marvell.c#L3749

    let sds_val = rtldev.read_reg(0x7b20)?;
    let sds0 = sds_val & 0x1f;
    let sds1 = (sds_val >> 5) & 0x1f;
    println!("SDS0 {sds0:1x} SDS1 {sds1:1x}");
    if sds1 == 0x02 {
        println!("Change SDS1 tp 1000 BASEx");
    }

    sds_pcs_config(
        &mut rtldev,
        0,
        PhyInterfaceT::PHY_INTERFACE_MODE_SGMII,
        true,
        false,
    )?;

    // return Ok(());

    // Set copper Page
    let reg = MII_MARVELL_PHY_PAGE;
    phy_write(&mut rtldev, reg, 0x00)?;
    let val = phy_read(&mut rtldev, reg)?;
    println!("Reg: {reg:02x} = {:04x}", val);

    // Ext
    let reg = 0x1b;
    phy_modify(&mut rtldev, reg, 1 << 12, 0)?;

    marvell_config_aneg(&mut rtldev)?;

    // Set fiber Page
    let reg = MII_MARVELL_PHY_PAGE;
    phy_write(&mut rtldev, reg, 0x01)?;
    let val = phy_read(&mut rtldev, reg)?;
    println!("Reg: {reg:02x} = {:04x}", val);

    genphy_restart_aneg(&mut rtldev)?;

    // Set copper Page
    let reg = MII_MARVELL_PHY_PAGE;
    phy_write(&mut rtldev, reg, 0x00)?;
    let val = phy_read(&mut rtldev, reg)?;
    println!("Reg: {reg:02x} = {:04x}", val);

    return Ok(());
    // let regs = [Reg::new("MAC_LINK_STS", 0x63e8)];

    // loop {
    //     for Reg { name, addr } in &regs {
    //         let val = rtldev.read_reg(*addr)?;
    //         println!(
    //             "[{addr:04x}] {name:40}: {:08x} {:032b} 1's {}",
    //             val,
    //             val,
    //             val.count_ones()
    //         );
    //         std::thread::sleep(Duration::from_secs(1));
    //     }
    // }

    // for (idx, &reg) in [Regs::IoMuxSel0, Regs::IoMuxSel1, Regs::IoMuxSel2]
    //     .iter()
    //     .enumerate()
    // {
    //     let val = rtldev.read_reg(reg.into())?;
    //     println!("IO MUX {idx}: {:08x?}", val);
    // }

    // for reg in [
    //     Regs::Gpio00_31Dir,
    //     Regs::Gpio32_63Dir,
    //     Regs::IoMuxSel0,
    //     Regs::IoMuxSel1,
    //     Regs::LedGlbMux1,
    //     Regs::LedGlbMux2,
    //     Regs::LedGlbMux3,
    //     Regs::LedGlbMux4,
    //     Regs::LedGlbMux5,
    //     Regs::LedGlbMux6,
    // ] {
    //     let reg: u16 = reg.into();
    //     if let Ok(val) = rtldev.read_reg(reg) {
    //         println!("{:04x}: {val:08x}", reg);
    //     };
    //     rtldev.write_reg(reg, 0x0)?;
    // }

    // let mut gpio0 = 0;
    // let mut gpio1 = 0;

    // let reg0 = Regs::Gpio00_31Input.into();
    // let reg1 = Regs::Gpio32_63Input.into();
    // loop {
    //     match rtldev.read_reg(reg0) {
    //         Ok(val) => {
    //             let diff = gpio0 ^ val;
    //             print!("{reg0:04x}={:08x?} {:08x?} ", val, diff);
    //             for bit in (0..31).rev() {
    //                 if ((1 << bit) & diff) != 0 {
    //                     print!(" GPIO{bit:02}");
    //                 }
    //             }
    //             println!();
    //             gpio0 = val;
    //         }
    //         Err(err) => println!("{reg0:04x}=Err({err:?}"),
    //     }
    //     match rtldev.read_reg(reg1) {
    //         Ok(val) => {
    //             let diff = gpio1 ^ val;
    //             print!("{reg1:04x}={:08x?} {:08x?} ", val, diff);
    //             for bit in (0..31).rev() {
    //                 if ((1 << bit) & diff) != 0 {
    //                     print!(" GPIO{:02}", bit + 32);
    //                 }
    //             }
    //             println!();
    //             gpio1 = val;
    //         }
    //         Err(err) => println!("{reg0:04x}=Err({err:?}"),
    //     }
    //     std::thread::sleep(Duration::from_millis(500));
    // }

    // rtldev
    //     .write_reg(Regs::LedGlbMux1.into(), 0x08144040)
    //     .unwrap();        Reg::new("LED_DUMY_0_ADDR", 0x6604),
    // Reg::new("LED_DUMY_1_ADDR", 0x6608),
    // rtldev
    //     .write_reg(Regs::LedGlbMux2.into(), 0x10349309)
    //     .unwrap();
    // rtldev
    //     .write_reg(Regs::LedGlbMux3.into(), 0x1245038d)
    //     .unwrap();
    // rtldev
    //     .write_reg(Regs::LedGlbMux4.into(), 0x19616555)
    //     .unwrap();
    // rtldev
    //     .write_reg(Regs::LedGlbMux3.into(), 0x1c79d65a)
    //     .unwrap();
    // rtldev
    //     .write_reg(Regs::LedGlbMux4.into(), 0x0002181d)
    //     .unwrap();

    // let values: [(u16, u32); _] = [
    //     // LED3_0_SET3_2_CTRL1
    //     // (0x6520, 0x0023e430),
    //     // (0x6524, 0xff001400),
    //     (0x6528, 0x00110000),
    //     // (0x652c, 0x007f013f),
    //     // (0x6530, 0x02000400),
    //     // (0x6534, 0x01400141),
    //     // (0x6538, 0x01440170),
    //     (0x653c, 0x18000041),
    //     (0x6540, 0x01400155),
    //     // (0x6544, 0x01411000),
    //     // (0x6548, 0x01750041),
    //     // LED_PORT_SET_SEL_CTRL
    //     // (0x654c, 0x00010000),

    //     // (0x6580, 0x000fa000),
    //     // (0x6584, 0x00271000),
    //     // (0x6588, 0x004e2000),
    //     // (0x658c, 0x000fa000),
    //     // (0x6590, 0x00271000),

    //     // (0x6594, 0x004e2000),
    //     // (0x6598, 0x000fa000),
    //     // (0x659c, 0x00271000),
    //     // (0x65a0, 0x004e2000),
    //     // (0x65a4, 0x000fa000),
    //     // (0x65a8, 0x00271000),
    //     // (0x65ac, 0x004e2000),
    //     // (0x65b0, 0x0007d000),
    //     // (0x65b4, 0x00138800),
    //     // (0x65b8, 0x00271000),
    //     // (0x65bc, 0x00019000),
    //     // (0x65c0, 0x0003e800),
    //     // (0x65c4, 0x0007d000),
    //     // (0x65c8, 0x00002800),
    //     // (0x65cc, 0x00006400),
    //     // (0x65d0, 0x0000c800),
    //     // (0x65d4, 0x00000000),
    //     // (0x65d8, 0x3ffb6dff),
    //     // (0x65dc, 0x7f24977f),
    //     // (0x65e0, 0x08144040),
    //     // (0x65e4, 0x10349309),
    //     // (0x65e8, 0x12454391),
    //     // (0x65ec, 0x19616555),
    //     // (0x65f0, 0x1c79d65a),
    //     // (0x65f4, 0x0002181d),
    //     // (0x65f8, 0x0000001b),
    //     // (0x65fc, 0x33333333),
    //     // (0x6600, 0x00000003),
    //     // (0x7f8c, 0x20db6880),
    //     // (0x7f90, 0x400002c1),
    // ];

    // let mut val = rtldev.read_reg(Regs::Gpio00_31Output.into())?;
    // println!("GPIO OUT {val:08x}");
    // val &= !(1 << 30);
    // rtldev.write_reg(Regs::Gpio00_31Output.into(), val)?;

    // let mut val = rtldev.read_reg(Regs::Gpio00_31Dir.into())?;
    // println!("GPIO DIR {val:08x}");
    // val |= 1 << 30;
    // rtldev.write_reg(Regs::Gpio00_31Dir.into(), val)?;

    // let mut val = rtldev.read_reg(Regs::Gpio00_31Output.into())?;
    // println!("GPIO OUT {val:08x}");
    // val |= 1 << 30;
    // rtldev.write_reg(Regs::Gpio00_31Output.into(), val)?;

    // for (reg, value) in values {
    //     rtldev.write_reg(reg, value).unwrap();
    // }
    //, 0xfor reg in (0..0xFFFF).step_by(4) {),
    //     if [0x10, 0x14, 0x18].contains(&reg) {
    //         println!("{reg:04x}=<Redacted>");
    //         continue;
    //     }
    //     match rtldev.read_reg(reg) {
    //         Ok(val) => println!("{reg:04x}={:08x?}", val),
    //         Err(err) => println!("{reg:04x}=Err({err:?}"),
    //     }
    // }

    // let mut val = rtldev.read_reg(0x65d8).unwrap();
    // val &= !(1 << 9);
    // // val &= !(1 << 24);
    // val |= 1 << 24;
    // val &= !(3 << 13);
    // val |= 3 << 13;
    // rtldev.write_reg(0x65d8, val).unwrap();

    // let new = rtldev.read_reg(0x65d8).unwrap();
    // if val != new {
    //     println!("No value change! {val:08x} != {new:08x}")
    // }

    let regs = [
        Reg::new("LED_GLB_MUX_1", 0x65e0),
        Reg::new("LED_GLB_MUX_2", 0x65e4),
        Reg::new("LED_GLB_MUX_3", 0x65e8),
        Reg::new("LED_GLB_MUX_4", 0x65ec),
        Reg::new("LED_GLB_MUX_5", 0x65f0),
        Reg::new("LED_GLB_MUX_6", 0x65f4),
        Reg::new("LED_GLB_ACTIVE", 0x65d8),
        Reg::new("LED_GLB_CTRL ", 0x6520),
        Reg::new("LED_RLDP_CTRL_1", 0x65f8),
        Reg::new("LED_RLDP_CTRL_2", 0x65fc),
        Reg::new("LED_RLDP_CTRL_3", 0x6600),
        Reg::new("LED_GLB_IO_EN", 0x65dc),
        Reg::new("IO_MUX_SEL_0", 0x7f8c),
        Reg::new("IO_MUX_SEL_1", 0x7F90),
        Reg::new("LED_PORT_SET_SEL_CTRL", 0x654c),
        Reg::new("LED3_0_SET3_2_CTRL1", 0x6524),
        Reg::new("LED3_0_SET1_0_CTRL1", 0x6528),
        Reg::new("LED3_2_SET3_CTRL0", 0x652C),
        Reg::new("LED1_0_SET3_CTRL0", 0x6530),
        Reg::new("LED3_2_SET2_CTRL0", 0x6534),
        Reg::new("LED1_0_SET2_CTRL0", 0x6538),
        Reg::new("LED3_2_SET1_CTRL0", 0x653C),
        Reg::new("LED1_0_SET1_CTRL0", 0x6540),
        Reg::new("LED3_2_SET0_CTRL0", 0x6544),
        Reg::new("LED1_0_SET0_CTRL0", 0x6548),
        Reg::new("SW_LED_LOAD", 0x6550),
        Reg::new("LED_PORT_SW_EN_CTRL_P0-6", 0x6554),
        Reg::new("LED_PORT_SW_EN_CTRL_P7-8", 0x6558),
        Reg::new("LED_PORT_SW_CTRL_P0", 0x655C),
        Reg::new("LED_PORT_SW_CTRL_P1", 0x6560),
        Reg::new("LED_PORT_SW_CTRL_P2", 0x6564),
        Reg::new("LED_PORT_SW_CTRL_P3", 0x6568),
        Reg::new("LED_PORT_SW_CTRL_P4", 0x656C),
        Reg::new("LED_PORT_SW_CTRL_P5", 0x6570),
        Reg::new("LED_PORT_SW_CTRL_P6", 0x6574),
        Reg::new("LED_PORT_SW_CTRL_P7", 0x6578),
        Reg::new("LED_PORT_SW_CTRL_P8", 0x657C),
        Reg::new("LED_LOAD_LV1_10G", 0x6580),
        Reg::new("LED_LOAD_LV2_10G", 0x6584),
        Reg::new("LED_LOAD_LV3_10G", 0x6588),
        Reg::new("LED_LOAD_LV1_5G", 0x658C),
        Reg::new("LED_LOAD_LV2_5G", 0x6590),
        Reg::new("LED_LOAD_LV3_5G", 0x6594),
        Reg::new("LED_LOAD_LV1_2P5G", 0x6598),
        Reg::new("LED_LOAD_LV2_2P5G", 0x659C),
        Reg::new("LED_LOAD_LV3_2P5G", 0x65A0),
        Reg::new("LED_LOAD_LV1_1G", 0x65A4),
        Reg::new("LED_LOAD_LV2_1G", 0x65A8),
        Reg::new("LED_LOAD_LV3_1G", 0x65AC),
        Reg::new("LED_LOAD_LV1_500M", 0x65B0),
        Reg::new("LED_LOAD_LV2_500M", 0x65B4),
        Reg::new("LED_LOAD_LV3_500M", 0x65B8),
        Reg::new("LED_LOAD_LV1_100M", 0x65BC),
        Reg::new("LED_LOAD_LV2_100M", 0x65C0),
        Reg::new("LED_LOAD_LV3_100M", 0x65C4),
        Reg::new("LED_LOAD_LV1_10M", 0x65C8),
        Reg::new("LED_LOAD_LV2_10M", 0x65CC),
        Reg::new("LED_LOAD_LV3_10M", 0x65D0),
        Reg::new("LED_P_LOAD_CTRL", 0x65D4),
        Reg::new("LED_DUMY_0_ADDR", 0x6604),
        Reg::new("LED_DUMY_1_ADDR", 0x6608),
    ];

    for Reg { name, addr } in regs {
        let Ok(mut val) = rtldev.read_reg(addr.into()) else {
            println!("[{addr:04x}] Error");
            continue;
        };
        println!(
            "[{addr:04x}] {name:40}: {:08x} {:032b} 1's {}",
            val,
            val,
            val.count_ones()
        );
        match addr {
            0x65e0..=0x65F4 => {
                // RTL8373_LED_GLB_MUX_1_ADDR
                let led_offset = ((addr - 0x65e0) >> 2) * 5;
                let n = if addr == 0x65F4 { 2 } else { 4 };

                let mut v = Vec::<u8>::with_capacity(6);
                for _ in 0..=n {
                    v.push((val & 0x3f) as u8);
                    val >>= 6;
                }

                for (val, led) in v.iter().zip(0_u16..) {
                    let led = led + led_offset;
                    println!("\tLed{led:2}: {:02x}", *val);
                }
            }
            0x654c => {
                for port in 0..=8 {
                    println!("\tport{port} PSEL: {} -- ", (val >> (port << 1)) & 0x3);
                }
            }

            // LED-sets
            0x6524..=0x6528 => {
                for set in 0..2 {
                    for led in 0..4 {
                        let value = (val & 0x0F) as u8;
                        print!(
                            "\tSET{} LED{led}: {value:x} -- ",
                            set + (u16::from(addr == 0x6524) << 1)
                        );
                        disp_led_high(value);
                        println!();
                        val >>= 4;
                    }
                }
            }

            //RTL8373_LED_GLB_ACTIVE_ADDR
            0x65d8 => {
                for led in 0..=29 {
                    println!(
                        "\tLed{led:2}: Active {}",
                        if val & (1 << led) != 0 { "Low" } else { "High" }
                    );
                }
            }
            // RTL8373_LED_GLB_IO_EN_ADDR
            0x65dc => {
                for led in 0..=29 {
                    println!(
                        "\tLed{led:2}: {}",
                        if val & (1 << led) != 0 { "On" } else { "Off" }
                    );
                }
                println!(
                    "\tLed Pad:  {}",
                    if val & (1 << 30) != 0 { "On" } else { "Off" }
                );
            }

            // RTL8373_IO_MUX_SEL_0_ADDR
            0x7F8C => {
                for led in 0..=27 {
                    println!(
                        "\tLed{led:2}: MUX SEL: {}",
                        if val & (1 << led) != 0 { "LED" } else { "GPIO" }
                    );
                }
                println!(
                    "\tSYS LED:  {}",
                    if val & (1 << 28) != 0 { "Alt" } else { "GPIO" }
                );
                println!(
                    "\tRLDP_LED:  {}",
                    if val & (1 << 29) != 0 { "Alt" } else { "GPIO" }
                );
            }
            //RTL8373_LED_GLB_CTRL_ADDR
            0x6520 => {
                println!("\tPWR ON BLINK SEL: {:x}", (val >> 3) & 0x3);
                println!("\tSTP1_PWR_ON_LED: {:x}", (val >> 5) & 0xF);
                println!("\tSTP2_PWR_ON_LED: {:x}", (val >> 9) & 0xF);
                println!("\tFIB_UNIDIR_LED_EN: {:x}", (val >> 14) & 0x1);
                println!("\tSYS_LED_EN: {:x}", (val >> 15) & 0x1);
                println!("\tSYS_LED_MODE: {:x}", (val >> 16) & 0x3);
            }

            //LED3_2_SET3_CTRL...
            0x652C..=0x6548 => {
                print!("\tL: {:04x} -- ", (val) & 0xFFFF);
                disp_led_low(val as u16);
                print!("\n\tM: {:04x} -- ", (val >> 16) & 0xFFFF);
                disp_led_low((val >> 16) as u16);
                println!();
            }

            _ => (),
        }
    }

    fn disp_led_low(val: u16) {
        if (val & 0x00001) != 0 {
            print!("2G5 ");
        }
        if (val & 0x00002) != 0 {
            print!("TWO_PAIR_1G ");
        }
        if (val & 0x00004) != 0 {
            print!("1G ");
        }
        if (val & 0x00008) != 0 {
            print!("500M ");
        }
        if (val & 0x00010) != 0 {
            print!("100M ");
        }
        if (val & 0x00020) != 0 {
            print!("10M ");
        }
        if (val & 0x00040) != 0 {
            print!("LINK ");
        }
        if (val & 0x00080) != 0 {
            print!("LINK_FLASH ");
        }
        if (val & 0x00100) != 0 {
            print!("ACT ");
        }
        if (val & 0x00200) != 0 {
            print!("RX ");
        }
        if (val & 0x00400) != 0 {
            print!("TX ");
        }
        if (val & 0x00800) != 0 {
            print!("COL ");
        }
        if (val & 0x01000) != 0 {
            print!("DUPLEX ");
        }
        if (val & 0x02000) != 0 {
            print!("TRAINING ");
        }
        if (val & 0x04000) != 0 {
            print!("MASTER ");
        }
    }

    fn disp_led_high(val: u8) {
        if (val & 0x1) != 0 {
            print!("LEDS_10G ");
        }
        if (val & 0x2) != 0 {
            print!("LEDS_TWO_PAIR_5G ");
        }
        if (val & 0x4) != 0 {
            print!("LEDS_5G ");
        }
        if (val & 0x8) != 0 {
            print!("LEDS_TWO_PAIR_2G5 ");
        }
    }

    // Enable SYS_LED
    let mut val = rtldev.read_reg(Regs::IoMuxSel0.into()).unwrap();
    val |= 1 << 28;
    rtldev.write_reg(Regs::IoMuxSel0.into(), val).unwrap();

    // let mut val = rtldev.read_reg(0x7f8c).unwrap();
    // val |= 1 << 28;
    // rtldev.write_reg(0x7f8c, val).unwrap();

    // Set Led mode

    // Mode 0 = Off
    // Mode 1 = Fast blinking 3 Hz
    // Mode 2 = Slow blinking 0.5 Hz
    // Mode 3 = On

    // Enable SYS_LED
    // for mode in 0..=3 {
    //     println!("Set mode: {mode}");
    //     let mut val = rtldev.read_reg(0x6520).unwrap();
    //     val &= !(3 << 16);
    //     val |= mode << 16;
    //     rtldev.write_reg(0x6520, val).unwrap();
    //     std::thread::sleep(Duration::from_secs(4));
    // }

    match rtldev.get_i2c_gpio_pin_group() {
        Ok((sda, scl)) => println!("I2c group sda={sda}, scl={scl}"),
        Err(err) => println!("Error: {err:?}"),
    }

    // match rtldev.rtl8224_reg_read(0x04) {
    //     Ok(val) => println!("Chip ID: {val:04x}"),
    //     Err(err) => println!("Error: {err:?}"),
    // }

    // let reg: u16 = 0x1210;
    // match rtldev.rtl8224_reg_read(reg) {
    //     Ok(val) => println!("{reg:04x}: {val:04x}"),
    //     Err(err) => println!("Error: {err:?}"),
    // }

    // rtldev.rtl8224_reg_write(reg, 0xAAAA5555).unwrap();

    // match rtldev.rtl8224_reg_read(reg) {
    //     Ok(val) => println!("{reg:04x}: {val:04x}"),
    //     Err(err) => println!("Error: {err:?}"),
    // }

    // rtldev.rtl8224_reg_write(reg, 0x5555AAAA).unwrap();

    // match rtldev.rtl8224_reg_read(reg) {
    //     Ok(val) => println!("{reg:04x}: {val:04x}"),
    //     Err(err) => println!("Error: {err:?}"),
    //}

    //dal_rtl8224_sds_regbits_write(0, 6, 2, 0x2000, 1); //##S0RX PN swap for 64B/66B

    // let mut val = rtldev.rtl8224_sds_reg_read(0, 6, 2).unwrap();
    // println!("SDS: {val:04x}");

    // val |= 0x2000;
    // rtldev.rtl8224_sds_reg_write(0, 6, 2, val).unwrap();

    // let val = rtldev.rtl8224_sds_reg_read(0, 6, 2).unwrap();
    // println!("SDS: {val:04x}");

    Ok(())
}
