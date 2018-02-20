use std::sync::mpsc;
use screen::Screen;

const VIDEO_RAM_SIZE: usize = 0x2000;

pub struct GPU {
    next_screen_buffer: Vec<u8>,
    video_ram: [u8; VIDEO_RAM_SIZE],
    bg_palette: u8,
    bg_palette_map: [u8; 4],
    obj_palette_0: u8,
    obj_palette_1: u8,
    oam: [u8; GPU::OAM_SIZE], // Sprite attribute table
    lcd_control: u8,
    stat: u8,
    scy: u8,
    scx: u8,
    win_y: u8,
    win_x: u8,
    ly: u8,
    render_clock: u32,
    screen_data_sender: mpsc::SyncSender<Vec<u8>>,
    pub interrupt: u8,
}

impl GPU {
    pub const OAM_SIZE: usize = 0xA0;

    pub fn new(screen_data_sender: mpsc::SyncSender<Vec<u8>>) -> Self {
        Self {
            next_screen_buffer: vec![0_u8; (3 * Screen::WIDTH * Screen::HEIGHT) as usize],
            video_ram: [0_u8; VIDEO_RAM_SIZE],
            bg_palette: 0,
            obj_palette_0: 0,
            obj_palette_1: 0,
            bg_palette_map: build_palette_map(0),
            oam: [0_u8; 160],
            lcd_control: 0x91,
            stat: 0,
            scy: 0,
            scx: 0,
            win_y: 0,
            win_x: 0,
            ly: 0,
            render_clock: 0,
            screen_data_sender,
            interrupt: 0,
        }
    }

    pub fn run_cycle(&mut self, cycles: u8) {
        if !self.is_lcd_on() {
            return
        }

        let mut cycles_to_process = cycles;

        while cycles_to_process > 0 {
            cycles_to_process -= self.process_cycles(u32::from(cycles_to_process));
        }
    }

    pub fn read_oam(&self, addr: u16) -> u8 {
        self.oam[(addr & 0xFF) as usize]
    }

    pub fn write_oam(&mut self, addr: u16, value: u8) {
        self.oam[(addr & 0xFF) as usize] = value;
    }

    pub fn read_video_ram(&self, addr: u16) -> u8 {
        self.video_ram[(addr & 0x1FFF) as usize]
    }

    pub fn write_video_ram(&mut self, addr: u16, value: u8) {
        self.video_ram[(addr & 0x1FFF) as usize] = value;
    }

    pub fn read_control(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.lcd_control,
            0xFF41 => self.stat,
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF46 => unreachable!("DMA Address is write only"),
            0xFF47 => self.bg_palette,
            0xFF48 => self.obj_palette_0,
            0xFF49 => self.obj_palette_1,
            0xFF4A => self.win_y,
            0xFF4B => self.win_x,
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
            0xFF46 => unreachable!("DMA write handled in mmu.rs"),
            0xFF47 => {
                self.bg_palette = value;
                self.bg_palette_map = build_palette_map(value);
            },
            0xFF48 => self.obj_palette_0 = value,
            0xFF49 => self.obj_palette_1 = value,
            0xFF4A => self.win_y = value,
            0xFF4B => self.win_x = value,
            _ => panic!("Unknown GPU control write operation: 0x{:X}", addr),
        }
    }

    fn process_cycles(&mut self, cycles: u32) -> u8 {
        if self.render_clock + cycles >= 114 {
            #[cfg_attr(feature="clippy", allow(cast_possible_truncation))]
            let used_cycles = (self.render_clock + cycles - 114) as u8;
            self.render_clock = 0;
            self.increment_line();
            self.render_background();
            used_cycles
        } else {
            self.render_clock += cycles;
            #[cfg_attr(feature="clippy", allow(cast_possible_truncation))]
            let cycles_u8 = cycles as u8;
            cycles_u8
        }
    }

    fn is_lcd_on(&self) -> bool {
        self.lcd_control & 0x80 > 0
    }

    fn increment_line(&mut self) {
        self.ly = (self.ly + 1) % 154;
        if self.ly >= 144 { // V-Blank
            if self.ly == 144 {
                self.interrupt |= 0x01; // Mark V-Blank interrupt
            }
            self.render_screen();
        }
    }

    fn render_background(&mut self) {
        if self.ly >= 144 {
            return
        }

//        let log_out = self.ly == 9;

        let bg_tile_map_addr = self.bg_tile_map_addr();
        let bg_tile_data_addr = self.bg_tile_data_addr();
        let bgy = self.scy.wrapping_add(self.ly);
        let bgy_tile = (u16::from(bgy) & 0xFF) >> 3;
        let bgy_pixel_in_tile = u16::from(bgy) & 0x07;

        for x in 0 .. Screen::WIDTH {
            let bgx = u32::from(self.scx) + x;
            #[cfg_attr(feature="clippy", allow(cast_possible_truncation))]
            let bgx_tile = ((bgx & 0xFF) >> 3) as u16;
            #[cfg_attr(feature="clippy", allow(cast_possible_truncation))]
            let bgx_pixel_in_tile = (bgx & 0x07) as u8;

            let tile_number_addr = bg_tile_map_addr + bgy_tile * 32 + bgx_tile;
            let tile_number: u8 = self.read_byte_video_ram(tile_number_addr);
//            if log_out {
//                println!("TILE_NUMBER_ADDR: 0x{:02X}", tile_number_addr);
//                println!("TILE_NUMBER: {}", tile_number);
//            }

            let tile_addr_offset = if bg_tile_data_addr == 0x8000 {
                // regular reading
                u16::from(tile_number) * 16
            } else {
                // reading with offset
                #[cfg_attr(feature="clippy", allow(cast_possible_truncation, cast_sign_loss, cast_possible_wrap))]
                let adjusted_tile_number = (i16::from(tile_number as i8) + 128) as u16;
                adjusted_tile_number * 16
            };
            let tile_addr = bg_tile_data_addr + tile_addr_offset;

            let tile_line_addr = tile_addr + bgy_pixel_in_tile * 2;
            let (tile_line_data_1, tile_line_data_2) = (self.read_byte_video_ram(tile_line_addr), self.read_byte_video_ram(tile_line_addr + 1));
            let pixel_in_line_mask = 1 << bgx_pixel_in_tile;
            let pixel_data_1: u8 = if tile_line_data_1 & pixel_in_line_mask > 0 {
                1
            } else {
                0
            };
            let pixel_data_2: u8 = if tile_line_data_2 & pixel_in_line_mask > 0 {
                2
            } else {
                0
            };

            let palette_color_id = pixel_data_1 | pixel_data_2;
            let color: u8 = self.bg_palette_map[palette_color_id as usize];

//            if log_out {
//                println!("TILE_ADDR: 0x{:02X}", tile_addr);
//                println!("BGY PIXEL: {}", bgy_pixel_in_tile);
//                println!("PIXEL DATA 1: {}", pixel_data_1);
//                println!("PIXEL DATA 1: {}", pixel_data_2);
//                println!("COLOR: {}", color);
//            }

            self.set_pixel_color_next_screen_buffer(x, color);
        }
    }

    fn bg_tile_data_addr(&self) -> u16 {
        if self.lcd_control & 0x10 > 0 {
            0x8000
        } else {
            0x8800
        }
    }

    fn bg_tile_map_addr(&self) -> u16 {
        if self.lcd_control & 0x08 > 0 {
            0x9C00
        } else {
            0x9800
        }
    }

    fn set_pixel_color_next_screen_buffer(&mut self, x_pixel: u32, color: u8) {
        let base_addr = (u32::from(self.ly) * Screen::WIDTH + x_pixel) as usize * 3;
        self.next_screen_buffer[base_addr] = color;
        self.next_screen_buffer[base_addr + 1] = color;
        self.next_screen_buffer[base_addr + 2] = color;
    }

    fn read_byte_video_ram(&self, addr: u16) -> u8 {
        self.video_ram[(addr & 0x1FFF) as usize]
    }

    fn render_screen(&self) {
        match self.screen_data_sender.send(self.next_screen_buffer.to_vec()) {
            Ok(_) => (),
            Err(e) => println!("Failed to send screen data: {}", e),
        };
    }
}

fn build_palette_map(palette_layout: u8) -> [u8; 4] {
    [
        color_from_dot_data(palette_layout & 0x11),
        color_from_dot_data((palette_layout >> 2) & 0x11),
        color_from_dot_data((palette_layout >> 4) & 0x11),
        color_from_dot_data(palette_layout >> 6),
    ]
}

fn color_from_dot_data(dot_data: u8) -> u8 {
    match dot_data {
        0x00 => 255,
        0x01 => 192,
        0x10 => 96,
        _ => 0,
    }
}
