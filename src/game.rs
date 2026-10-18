//! Native game state and frame scheduling for the statically translated routines.
//! There is no instruction fetch, opcode decoding, CPU clock or ROM interpreter.
use crate::{Controllers, audio::Audio, video::Video};
use std::path::Path;

mod translated {
    include!(concat!(env!("OUT_DIR"), "/native_game.rs"));
}

pub struct Game {
    pub memory: Box<[u8; 65536]>,
    pub video: Video,
    pub audio: Option<Audio>,
    controllers: Controllers,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    sp: u8,
    c: bool,
    z: bool,
    n: bool,
    v: bool,
    interrupt_mask: bool,
    decimal: bool,
    block: u16,
    frame_ready: bool,
    pub frame: u64,
    pub blocks_executed: u64,
    #[cfg(test)]
    test_start: Option<(u8, u8, u8)>,
    #[cfg(test)]
    test_started: bool,
}

impl Game {
    pub fn new(assets: &Path, sound: bool) -> Self {
        let mut memory = Box::new([0; 65536]);
        memory[0x8000..]
            .copy_from_slice(include_bytes!(concat!(env!("OUT_DIR"), "/game-data.bin")));
        let audio = if sound {
            match Audio::new(assets) {
                Ok(audio) => Some(audio),
                Err(error) => {
                    eprintln!("Audio unavailable: {error}");
                    None
                }
            }
        } else {
            None
        };
        Self {
            memory,
            video: Video::new(include_bytes!(concat!(env!("OUT_DIR"), "/tiles.bin"))),
            audio,
            controllers: Controllers::default(),
            a: 0,
            x: 0,
            y: 0,
            sp: 0,
            c: false,
            z: false,
            n: false,
            v: false,
            interrupt_mask: false,
            decimal: false,
            block: 0x8f72,
            frame_ready: false,
            frame: 0,
            blocks_executed: 0,
            #[cfg(test)]
            test_start: None,
            #[cfg(test)]
            test_started: false,
        }
    }

    pub fn buttons(&mut self, masks: [u8; 4]) {
        for (i, value) in masks.into_iter().enumerate() {
            self.controllers.clear_buttons(i, u32::MAX).unwrap();
            self.controllers.set_buttons(i, value.into()).unwrap();
        }
    }

    /// The original main routine waits at $8fc0 for its frame flag ($80).
    /// We call its recovered frame routine directly once per native update.
    pub fn update(&mut self) -> Result<(), String> {
        self.frame_ready = false;
        let mut sent_frame = false;
        for _ in 0..200_000 {
            if ((self.block == 0x8fc0 || self.block == 0xbf3c) && self.memory[0x80] == 0)
                || (self.block == 0xbf45 && self.memory[0x82] != 0)
            {
                if sent_frame {
                    return Err("Frame routine returned without its frame marker".into());
                }
                if self.block == 0xbf45 {
                    self.memory[0x80] = 0;
                }
                self.video.set_vblank(true);
                self.push_word(self.block);
                self.push(self.flags() & !0x10);
                self.interrupt_mask = true;
                self.block = 0xbf86;
                sent_frame = true;
            }
            let block = self.block;
            self.block = translated::dispatch(self, block)?;
            self.blocks_executed += 1;
            if self.frame_ready {
                self.frame += 1;
                if let Some(audio) = &mut self.audio {
                    audio.update();
                }
                return Ok(());
            }
        }
        Err(format!(
            "Game failed to reach its frame boundary: block ${:04x}, flag80={:02x}, flag81={:02x}",
            self.block, self.memory[0x80], self.memory[0x81]
        ))
    }

    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0..=0x1fff => self.memory[(addr & 0x7ff) as usize],
            0x2002 => {
                // Boot's vblank synchronization is satisfied by the native host.
                // In-game frame scheduling uses the recovered $80 handshake above.
                self.video.set_vblank(true);
                self.video.read_register(addr)
            }
            0x2000..=0x2007 => self.video.read_register(addr),
            0x4016 => self.controllers.read_port(0).unwrap(),
            0x4017 => self.controllers.read_port(1).unwrap(),
            0x8000..=0xffff => self.memory[addr as usize],
            _ => 0xff,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0..=0x1fff => self.memory[(addr & 0x7ff) as usize] = value,
            0x2000..=0x2007 => self.video.write_register(addr, value),
            0x4014 => {
                let base = (value as usize * 256) & 0x7ff;
                self.video.dma(&self.memory[base..base + 256]);
            }
            0x4016 => self.controllers.write_strobe(value),
            _ => {}
        }
    }

    fn pointer(&self, address: u8) -> u16 {
        self.memory[address as usize] as u16
            | ((self.memory[address.wrapping_add(1) as usize] as u16) << 8)
    }
    fn nz(&mut self, value: u8) {
        self.n = value & 128 != 0;
        self.z = value == 0;
    }
    fn add(&mut self, value: u8) {
        let result = self.a as u16 + value as u16 + self.c as u16;
        self.v = (!(self.a ^ value) & (self.a ^ result as u8) & 0x80) != 0;
        self.c = result > 255;
        self.a = result as u8;
        self.nz(self.a);
    }
    fn push(&mut self, value: u8) {
        self.memory[0x100 + self.sp as usize] = value;
        self.sp = self.sp.wrapping_sub(1);
    }
    fn pop(&mut self) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        self.memory[0x100 + self.sp as usize]
    }
    fn push_word(&mut self, value: u16) {
        self.push((value >> 8) as u8);
        self.push(value as u8);
    }
    fn pop_word(&mut self) -> u16 {
        let low = self.pop() as u16;
        low | (self.pop() as u16) << 8
    }
    fn flags(&self) -> u8 {
        self.c as u8
            | ((self.z as u8) << 1)
            | ((self.interrupt_mask as u8) << 2)
            | ((self.decimal as u8) << 3)
            | 0x20
            | ((self.v as u8) << 6)
            | ((self.n as u8) << 7)
    }
    fn set_flags(&mut self, value: u8) {
        self.c = value & 1 != 0;
        self.z = value & 2 != 0;
        self.interrupt_mask = value & 4 != 0;
        self.decimal = value & 8 != 0;
        self.v = value & 64 != 0;
        self.n = value & 128 != 0;
    }
    fn on_transfer(&mut self, target: u16) {
        #[cfg(test)]
        if target == 0xb317
            && let Some((level, players, mode)) = self.test_start.take()
        {
            // Same new-game entry used by the recovered menu/password code.
            self.memory[0xbc] = level;
            self.memory[3] = players - 1;
            self.memory[0x6f] = mode;
            self.test_started = true;
        }
        if let Some(audio) = &mut self.audio {
            audio.hook(target, self.a, self.x, self.y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_levels_and_bosses_execute_native_callbacks() {
        for mode in [0, 0x80] {
            for level in 0..14 {
                let mut game = Game::new(Path::new(env!("MICRO_MAGES_BUILD_ASSETS")), false);
                game.test_start = Some((level, 4, mode));
                let mut random = 0x12345678u32;
                for frame in 0..1800 {
                    let mut buttons = [0; 4];
                    if (120..126).contains(&frame) || (240..246).contains(&frame) {
                        buttons[0] = 8;
                    }
                    if frame > 500 {
                        random ^= random << 13;
                        random ^= random >> 17;
                        random ^= random << 5;
                        for (i, mask) in buttons.iter_mut().enumerate() {
                            let bits = (random >> (i * 8)) as u8;
                            *mask = (bits & 3) | if bits & 4 != 0 { 0x40 } else { 0x80 };
                        }
                    }
                    game.buttons(buttons);
                    game.update().unwrap_or_else(|e| {
                        panic!("mode {mode:02x}, level {level}, frame {frame}: {e}")
                    });
                }
                assert!(
                    game.test_started,
                    "new-game entry not reached for stage {level}"
                );
                assert!(game.video.render().iter().any(|&pixel| pixel != 0));
            }
        }
    }
}
