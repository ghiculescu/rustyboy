use std::fs::File;
use std::io::Read;
use std::sync::mpsc;
use gpu::GPU;
use serial::Serial;

// Gameboy only needs 0x2000 working RAM
// In the future if CGB support is needed,
// this should be expanded to 0x8000 to support
// the switchable memory modules.
const WRAM_SIZE: usize = 0x2000;
const ZRAM_SIZE: usize = 0x80;

pub struct MMU {
    rom: Vec<u8>,
    wram: [u8; WRAM_SIZE], // Working RAM
    zram: [u8; ZRAM_SIZE], // Zero page RAM
    gpu: GPU,
    serial: Serial,
}

impl MMU {
    pub fn new(cart_path: &str, screen_data_sender: mpsc::SyncSender<Vec<u8>>) -> Self {
        let mut cart_data: Vec<u8> = Vec::new();
        Self::load_cart(cart_path, &mut cart_data);

        Self {
            rom: cart_data,
            wram: [0_u8; WRAM_SIZE],
            zram: [0_u8; ZRAM_SIZE],
            gpu: GPU::new(screen_data_sender),
            serial: Serial::new(),
        }
    }

    pub fn run_cycle(&mut self, cpu_cycles: u8) {
        self.gpu.run_cycle(cpu_cycles)
    }

    // http://marc.rawer.de/Gameboy/Docs/GBCPUman.pdf
    pub fn read_byte(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000...0x7FFF => self.rom[addr as usize], // ROM
            0x8000...0x9FFF => self.gpu.read_video_ram(addr), // Load from GPU
            0xA000...0xBFFF => panic!("MMU ERROR: Load from cart RAM not implemented"), // Load from cartridge RAM
            0xC000...0xFDFF => self.wram[(addr & 0x1FFF) as usize], // Working RAM
            0xFE00...0xFE9F => self.gpu.read_oam(addr), // Graphics - sprite information
//            0xFF00 => 0, // Input read
            0xFF01...0xFF02 => self.serial.read(addr), // Serial read
//            0xFF04 => 0, // Div register
//            0xFF05...0xFF07 => 0, // Timer counter, modulo and control
//            0xFF0F => 0, // Interrupt flag
//            0xFF10...0xFF26 => 0, // Sound control
//            0xFF30...0xFF3F => 0, // Sound wave pattern RAM
            0xFF40...0xFF4B => self.gpu.read_control(addr),
//            0xFF4C...0xFF7F => panic!("MMU ERROR: Memory mapped I/O (read) (CGB only) not implemented"), // Memory mapped I/O CGB ONLY
            0xFF80...0xFFFF => self.zram[(addr & 0x7F) as usize], // Zero page RAM
            _ => 0,
        }
    }

    pub fn read_word(&mut self, addr: u16) -> u16 {
        u16::from(self.read_byte(addr)) | (u16::from(self.read_byte(addr + 1)) << 8)
    }

    // http://marc.rawer.de/Gameboy/Docs/GBCPUman.pdf
    pub fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000...0x7FFF => self.rom[addr as usize] = value, // ROM
            0x8000...0x9FFF => self.gpu.write_video_ram(addr, value), // Write to GPU
            0xA000...0xBFFF => panic!("MMU ERROR: Write to cart RAM not implemented"), // Write to cartridge RAM
            0xC000...0xFDFF => self.wram[(addr & 0x1FFF) as usize] = value, // Working RAM
            0xFE00...0xFE9F => self.gpu.write_oam(addr, value), // Graphics - sprite information
//            0xFF00 => (), // Input write
            0xFF01...0xFF02 => self.serial.write(addr, value), // Serial write
//            0xFF04 => (), // Div register
//            0xFF05...0xFF07 => (), // Timer counter, modulo and control
//            0xFF0F => (), // Interrupt flag
//            0xFF10...0xFF26 => (), // Sound control
//            0xFF30...0xFF3F => (), // Sound wave pattern RAM
            0xFF40...0xFF4B => self.gpu.write_control(addr, value),
//            0xFF4C...0xFF7F => panic!("MMU ERROR: Memory mapped I/O (write) (CGB only) not implemented"), // Memory mapped I/O CGB ONLY
            0xFF80...0xFFFF => self.zram[(addr & 0x7F) as usize] = value, // Zero page RAM
            _ => (),
        }
    }

    pub fn write_word(&mut self, addr: u16, value: u16) {
        self.write_byte(addr, (value & 0xFF) as u8);
        self.write_byte(addr + 1, (value >> 8) as u8);
    }

    fn load_cart(cart_path: &str, buffer: &mut Vec<u8>) {
        let mut file = match File::open(cart_path) {
            Ok(f) => f,
            Err(e) => panic!("Failed to open file from {}: {}", cart_path, e),
        };

        match file.read_to_end(buffer) {
            Ok(_) => println!("ROM loaded from {}", &cart_path),
            Err(e) => panic!("Failed to read file from {}: {}", cart_path, e),
        }
    }
}
