const SLAVE_ADDR: u8 = 0x5c;
use std::time::Duration;

use rtl837x::Regs;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "/dev/i2c-7";
    let mut bus = match linux_embedded_hal::I2cdev::new(path) {
        Ok(bus) => bus,
        Err(e) => {
            println!("Error opening I2C Bus {} {:?}", path, e);
            return Err(e.into());
        }
    };

    let mut rtldev = rtl837x::Rtl837x::new(&mut bus, SLAVE_ADDR);

    let soc = rtldev.get_chip_id()?;
    println!("SOC: {soc:?}");

    let soc = rtldev.get_soc_version()?;
    println!("SOC_REVISION: {soc:03x}");
    let soc = rtldev.get_soc_version()?;
    println!("SOC_REVISION: {soc:03x}");

    // for (idx, &reg) in [Regs::IoMuxSel0, Regs::IoMuxSel1, Regs::IoMuxSel2]
    //     .iter()
    //     .enumerate()
    // {
    //     let val = rtldev.read_reg(reg.into())?;
    //     println!("IO MUX {idx}: {:08x?}", val);
    // }

    // for reg in (0..0xFFFF).step_by(4) {
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

    struct Reg {
        name: &'static str,
        addr: u16,
    }

    impl Reg {
        fn new(name: &'static str, addr: u16) -> Self {
            Self { name, addr }
        }
    }

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

    // for Reg { name, addr } in regs {
    //     let Ok(mut val) = rtldev.read_reg(addr.into()) else {
    //         println!("[{addr:04x}] Error");
    //         continue;
    //     };
    //     println!(
    //         "[{addr:04x}] {name:40}: {:08x} {:032b} 1's {}",
    //         val,
    //         val,
    //         val.count_ones()
    //     );
    //     match addr {
    //         0x65e0..=0x65F4 => {
    //             // RTL8373_LED_GLB_MUX_1_ADDR
    //             let led_offset = ((addr - 0x65e0) >> 2) * 5;
    //             let n = if addr == 0x65F4 { 2 } else { 4 };
    //             for led in 0..=n {
    //                 let led = led + led_offset;
    //                 println!("\tLed{led:2}: {:02x}", val & 0x3f);
    //                 val >>= 6;
    //             }
    //         }
    //         0x654c => {
    //             for port in 0..=8 {
    //                 println!("\tport{port} PSEL: {}", (val >> (port << 1)) & 0x3);
    //             }
    //         }
    //         0x6524..=0x6528 => {
    //             for set in 0..2 {
    //                 for led in 0..4 {
    //                     println!(
    //                         "\tSET{} LED{led}: {:x}",
    //                         set + (u16::from(addr == 0x6524) << 1),
    //                         val & 0x0F
    //                     );
    //                     val >>= 4;
    //                 }
    //             }
    //         }

    //         //RTL8373_LED_GLB_ACTIVE_ADDR
    //         0x65d8 => {
    //             for led in 0..=29 {
    //                 println!(
    //                     "\tLed{led:2}: Active {}",
    //                     if val & (1 << led) != 0 { "Low" } else { "High" }
    //                 );
    //             }
    //         }
    //         // RTL8373_LED_GLB_IO_EN_ADDR
    //         0x65dc => {
    //             for led in 0..=29 {
    //                 println!(
    //                     "\tLed{led:2}: {}",
    //                     if val & (1 << led) != 0 { "On" } else { "Off" }
    //                 );
    //             }
    //             println!(
    //                 "\tLed Pad:  {}",
    //                 if val & (1 << 30) != 0 { "On" } else { "Off" }
    //             );
    //         }

    //         // RTL8373_IO_MUX_SEL_0_ADDR
    //         0x7F8C => {
    //             for led in 0..=27 {
    //                 println!(
    //                     "\tLed{led:2}: MUX SEL: {}",
    //                     if val & (1 << led) != 0 { "LED" } else { "GPIO" }
    //                 );
    //             }
    //             println!(
    //                 "\tSYS LED:  {}",
    //                 if val & (1 << 28) != 0 { "Alt" } else { "GPIO" }
    //             );
    //             println!(
    //                 "\tRLDP_LED:  {}",
    //                 if val & (1 << 29) != 0 { "Alt" } else { "GPIO" }
    //             );
    //         }
    //         //RTL8373_LED_GLB_CTRL_ADDR
    //         0x6520 => {
    //             println!("\tPWR ON BLINK SEL: {:x}", (val >> 3) & 0x3);
    //             println!("\tSTP1_PWR_ON_LED: {:x}", (val >> 5) & 0xF);
    //             println!("\tSTP2_PWR_ON_LED: {:x}", (val >> 9) & 0xF);
    //             println!("\tFIB_UNIDIR_LED_EN: {:x}", (val >> 14) & 0x1);
    //             println!("\tSYS_LED_EN: {:x}", (val >> 15) & 0x1);
    //             println!("\tSYS_LED_MODE: {:x}", (val >> 16) & 0x3);
    //         }
    //         0x652C..=0x6548 => {
    //             println!("\tL: {:04x}", (val) & 0xFFFF);
    //             println!("\tM: {:04x}", (val >> 16) & 0xFFFF);
    //         }

    //         _ => (),
    //     }
    // }

    // // Enable SYS_LED
    // let mut val = rtldev.read_reg(0x6520).unwrap();
    // val |= 1 << 15;
    // rtldev.write_reg(0x6520, val).unwrap();

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

    match rtldev.rtl8224_reg_read(0x04) {
        Ok(val) => println!("Chip ID: {val:04x}"),
        Err(err) => println!("Error: {err:?}"),
    }

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

    for _ in 0..100 {
        let mut val = rtldev.rtl8224_sds_reg_read(0, 6, 2).unwrap();
        println!("SDS: {val:04x}");
        if val == 0xe45c {
            continue;
        }
        val |= 0x2000;
        rtldev.rtl8224_sds_reg_write(0, 6, 2, val).unwrap();

        let val2 = rtldev.rtl8224_sds_reg_read(0, 6, 2).unwrap();
        if val != val2 {
            println!("SDS: {val:04x}");
            // break;
        }
    }

    Ok(())
}
