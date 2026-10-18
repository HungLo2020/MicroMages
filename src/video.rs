//! Native tile compositor reconstructed from the PC wrapper's Ppu and Renderer.
//!
//! This intentionally follows the shipped PC register contract, not NES timing:
//! data writes increment by one, status is the only readable register, and the
//! renderer composites complete frames without scanline or dot emulation.

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;

// RGB table at executable VA 0x439260 (file offset 0x37460).
const COLORS: [u32; 64] = [
    0x585858, 0x00237c, 0x0d1099, 0x300092, 0x4f006c, 0x600035, 0x5c0500, 0x461800, 0x272d00,
    0x093e00, 0x004500, 0x004106, 0x003545, 0x000000, 0x000000, 0x000000, 0xa1a1a1, 0x0b53d7,
    0x3337fe, 0x6621f7, 0x9515be, 0xac166e, 0xa62721, 0x864300, 0x596200, 0x2d7a00, 0x0c8500,
    0x007f2a, 0x006d85, 0x000000, 0x000000, 0x000000, 0xffffff, 0x51a5fe, 0x8084fe, 0xbc6afe,
    0xf15bfe, 0xfe5ec4, 0xfe7269, 0xe19321, 0xadb600, 0x79d300, 0x51df21, 0x3ad974, 0x39c3df,
    0x424242, 0x000000, 0x000000, 0xffffff, 0xb5d9fe, 0xcacafe, 0xe3befe, 0xf9b8fe, 0xfebae7,
    0xfec3bc, 0xf4d199, 0xdee086, 0xc6ec87, 0xb2f29d, 0xa7f0c3, 0xa8e7f0, 0xacacac, 0x000000,
    0x000000,
];

pub struct Video {
    pub control: u8,
    pub status: u8,
    pub mask: u8,
    pub scroll_x: u8,
    pub scroll_y: u8,
    pub oam: [u8; 256],
    memory: Box<[u8; 0x4000]>,
    oam_address: u8,
    address: u16,
    second_write: bool,
    mirror_mask: usize,
    pixels: Vec<u32>,
}

impl Video {
    pub fn new(chr: &[u8]) -> Self {
        let mut memory = Box::new([0; 0x4000]);
        let len = chr.len().min(0x2000);
        memory[..len].copy_from_slice(&chr[..len]);
        memory[0x3f00..0x3f20].fill(0x0f);
        Self {
            control: 0,
            status: 0,
            mask: 0,
            scroll_x: 0,
            scroll_y: 0,
            oam: [0; 256],
            memory,
            oam_address: 0,
            address: 0,
            second_write: false,
            mirror_mask: 0x400,
            pixels: vec![0; WIDTH * HEIGHT],
        }
    }

    pub fn set_mirroring(&mut self, vertical: bool) {
        self.mirror_mask = if vertical { 0x800 } else { 0x400 };
    }

    pub fn set_vblank(&mut self, active: bool) {
        if active {
            self.status |= 0x80;
        } else {
            self.status &= !0x80;
        }
    }

    pub fn read_register(&mut self, address: u16) -> u8 {
        if address == 0x2002 {
            let value = self.status;
            self.status &= 0x7f;
            self.second_write = false;
            value
        } else {
            0xff
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address {
            0x2000 => self.control = value,
            0x2001 => self.mask = value,
            0x2003 => self.oam_address = value,
            0x2005 => {
                if self.second_write {
                    self.scroll_y = value;
                } else {
                    self.scroll_x = value;
                }
                self.second_write = !self.second_write;
            }
            0x2006 => {
                if self.second_write {
                    self.address |= u16::from(value);
                } else {
                    self.address = u16::from(value) << 8;
                }
                self.second_write = !self.second_write;
            }
            0x2007 => {
                let index = self.memory_index(self.address as usize);
                self.memory[index] = value;
                self.address = self.address.wrapping_add(1);
            }
            _ => {}
        }
    }

    pub fn dma(&mut self, bytes: &[u8]) {
        for (offset, &value) in bytes.iter().take(256).enumerate() {
            self.oam[(self.oam_address as usize + offset) & 255] = value;
        }
    }

    fn memory_index(&self, address: usize) -> usize {
        let mirrored = if (0x2000..0x3000).contains(&address) {
            address & !self.mirror_mask
        } else {
            address
        };
        mirrored & 0x3fff
    }

    fn read(&self, address: usize) -> u8 {
        self.memory[self.memory_index(address)]
    }

    fn color(&self, address: usize) -> u32 {
        COLORS[(self.read(address) & 63) as usize]
    }

    fn tile_pixel(&self, address: usize, x: usize, y: usize) -> usize {
        let bit = 7 - x;
        (((self.read(address + y) >> bit) & 1) | (((self.read(address + y + 8) >> bit) & 1) << 1))
            as usize
    }

    fn sprites(&mut self, behind: bool) {
        let bank = ((self.control as usize & 8) << 9) & 0x1000;
        for sprite in (0..64).rev() {
            let i = sprite * 4;
            let y = self.oam[i] as usize + 1;
            let tile = self.oam[i + 1] as usize;
            let attributes = self.oam[i + 2];
            let x = self.oam[i + 3] as usize;
            if y >= HEIGHT || (attributes & 0x20 != 0) != behind {
                continue;
            }
            for dy in 0..8 {
                if y + dy >= HEIGHT {
                    break;
                }
                let ty = if attributes & 0x80 != 0 { 7 - dy } else { dy };
                for dx in 0..8 {
                    if x + dx >= WIDTH {
                        break;
                    }
                    let tx = if attributes & 0x40 != 0 { 7 - dx } else { dx };
                    let color = self.tile_pixel(bank + tile * 16, tx, ty);
                    if color != 0 {
                        self.pixels[(y + dy) * WIDTH + x + dx] =
                            self.color(0x3f10 + (attributes as usize & 3) * 4 + color);
                    }
                }
            }
        }
    }

    pub fn render(&mut self) -> &[u32] {
        let backdrop = self.color(0x3f00);
        self.pixels.fill(backdrop);
        self.sprites(true);
        let scroll_x = self.scroll_x as usize + (self.control as usize & 1) * 256;
        let scroll_y = self.scroll_y as usize + ((self.control as usize >> 1) & 1) * 240;
        let bank = (self.control as usize & 0x10) << 8;
        for y in 0..HEIGHT {
            let world_y = (y + scroll_y) % 480;
            let ty = (world_y % 240) / 8;
            for x in 0..WIDTH {
                let world_x = (x + scroll_x) % 512;
                let tx = (world_x % 256) / 8;
                let nametable = 0x2000 + (world_y / 240) * 0x800 + (world_x / 256) * 0x400;
                let tile = self.read(nametable + ty * 32 + tx) as usize;
                let color = self.tile_pixel(bank + tile * 16, world_x % 8, world_y % 8);
                if color != 0 {
                    let attribute = self.read(nametable + 0x3c0 + (ty / 4) * 8 + tx / 4);
                    let shift = ((ty & 2) << 1) | (tx & 2);
                    let palette = ((attribute >> shift) & 3) as usize;
                    self.pixels[y * WIDTH + x] = self.color(0x3f00 + palette * 4 + color);
                }
            }
        }
        self.sprites(false);
        &self.pixels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(video: &mut Video, address: u16, bytes: &[u8]) {
        video.read_register(0x2002);
        video.write_register(0x2006, (address >> 8) as u8);
        video.write_register(0x2006, address as u8);
        for &byte in bytes {
            video.write_register(0x2007, byte);
        }
    }

    #[test]
    fn pc_data_increment_and_nametable_alias_match_wrapper() {
        let mut video = Video::new(&[]);
        video.write_register(0x2000, 4);
        put(&mut video, 0x2000, &[12, 34]);
        assert_eq!(video.read(0x2400), 12);
        assert_eq!(video.read(0x2401), 34);
        assert_eq!(video.read(0x2020), 0);
        video.set_mirroring(true);
        assert_eq!(video.read(0x2800), 12);
        video.set_vblank(true);
        assert_eq!(video.read_register(0x2002), 0x80);
        assert_eq!(video.read_register(0x2002), 0);
    }

    #[test]
    fn tile_scroll_sprite_flip_and_priority_compose_correctly() {
        let mut chr = [0; 8192];
        chr[16] = 0x80; // tile one: leftmost pixel on first row
        let mut video = Video::new(&chr);
        video.oam.fill(255);
        put(&mut video, 0x2000, &[1]);
        put(&mut video, 0x3f00, &[15, 1]);
        put(&mut video, 0x3f11, &[0x21]);
        assert_eq!(video.render()[0], COLORS[1]);
        video.write_register(0x2005, 1);
        video.write_register(0x2005, 0);
        assert_eq!(video.render()[0], 0);
        video.write_register(0x2005, 0);
        video.write_register(0x2005, 0);
        video.oam[..4].copy_from_slice(&[0, 1, 0xc0, 3]);
        assert_eq!(video.render()[8 * WIDTH + 10], COLORS[0x21]);
        // Sprite behind background is covered only by nonzero background pixels.
        put(&mut video, 0x2020, &[1]);
        video.oam[..4].copy_from_slice(&[7, 1, 0x20, 0]);
        assert_eq!(video.render()[8 * WIDTH], COLORS[1]);
        video.oam[2] = 0;
        assert_eq!(video.render()[8 * WIDTH], COLORS[0x21]);
    }

    #[test]
    fn dma_wraps_at_oam_address() {
        let mut video = Video::new(&[]);
        video.write_register(0x2003, 254);
        video.dma(&[1, 2, 3, 4]);
        assert_eq!(&video.oam[254..], &[1, 2]);
        assert_eq!(&video.oam[..2], &[3, 4]);
    }
}
