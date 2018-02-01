pub struct GPU {
    oam: [u8; 160], // Sprite attribute table
    lcd_control: u8,
    stat: u8,
    scy: u8,
    scx: u8,
    ly: u8,
}

impl GPU {
    pub fn new() -> GPU {
        GPU {
            oam: [0u8; 160],
            lcd_control: 0,
            stat: 0,
            scy: 0,
            scx: 0,
            ly: 0,
        }
    }

    pub fn read_oam(&self, addr: u16) -> u8 {
        self.oam[(addr & 0xFF) as usize]
    }

    pub fn read_control(&mut self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.lcd_control,
            0xFF41 => self.stat,
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            _ => panic!("Unknown GPU control read operation: 0x{:X}", addr),
        }
    }

    pub fn write_control(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF40 => self.lcd_control = value,
            0xFF41 => self.stat = value,
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => (), // read only
            _ => panic!("Unknown GPU control write operation: 0x{:X}", addr),
        }
    }
}
