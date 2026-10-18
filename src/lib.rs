//! Native reconstruction of the installed Micro Mages PC game.
pub mod audio;
pub mod game;
pub mod input;
pub mod video;

/// Translation of mvm::Util::generateRomChecksum at VA 0x4014f0.
pub fn rom_checksum(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(0u32, |sum, &byte| sum.wrapping_add(byte.into()))
}

/// Controller behavior recovered from 0x411590..0x41173b.
/// This wrapper exposes players 3/4 on bit 1 of the two controller ports.
#[derive(Default)]
pub struct Controllers {
    buttons: [u32; 4],
    latched: [u32; 4],
    strobe: bool,
}

impl Controllers {
    pub fn set_buttons(&mut self, player: usize, mask: u32) -> Result<(), &'static str> {
        *self.buttons.get_mut(player).ok_or("player must be 0..4")? |= mask;
        Ok(())
    }

    pub fn clear_buttons(&mut self, player: usize, mask: u32) -> Result<(), &'static str> {
        *self.buttons.get_mut(player).ok_or("player must be 0..4")? &= !mask;
        Ok(())
    }

    pub fn write_strobe(&mut self, value: u8) {
        self.strobe = value & 1 != 0;
        // Original snapshots all four players on every write, including repeated zeroes.
        self.latched = self.buttons;
    }

    /// Returns the low byte of the original register result.
    pub fn read_port(&mut self, port: usize) -> Result<u8, &'static str> {
        if port > 1 {
            return Err("port must be 0 or 1");
        }
        let values = if self.strobe {
            &self.buttons
        } else {
            &self.latched
        };
        let result = 0xe0 | (values[port] as u8 & 1) | ((values[port + 2] as u8 & 1) << 1);
        if !self.strobe {
            self.latched[port] >>= 1;
            self.latched[port + 2] >>= 1;
        }
        Ok(result)
    }
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, &'static str> {
    let v = bytes
        .get(offset..offset.checked_add(2).ok_or("offset overflow")?)
        .ok_or("truncated u16")?;
    Ok(u16::from_le_bytes([v[0], v[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, &'static str> {
    let v = bytes
        .get(offset..offset.checked_add(4).ok_or("offset overflow")?)
        .ok_or("truncated u32")?;
    Ok(u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
}

/// Resolve a PE32 virtual address through section headers; never treat a VA as a file offset.
pub fn pe_slice(bytes: &[u8], va: u32, length: usize) -> Result<&[u8], &'static str> {
    if bytes.get(..2) != Some(b"MZ") {
        return Err("not a DOS/PE image");
    }
    let pe = u32_at(bytes, 0x3c)? as usize;
    if bytes.get(pe..pe.checked_add(4).ok_or("offset overflow")?) != Some(b"PE\0\0") {
        return Err("invalid PE signature");
    }
    if u16_at(bytes, pe + 4)? != 0x14c {
        return Err("expected x86 image");
    }
    let count = u16_at(bytes, pe + 6)? as usize;
    let optional_size = u16_at(bytes, pe + 20)? as usize;
    let optional = pe + 24;
    if optional_size < 32 || u16_at(bytes, optional)? != 0x10b {
        return Err("expected PE32");
    }
    let rva = va
        .checked_sub(u32_at(bytes, optional + 28)?)
        .ok_or("VA below image base")?;
    for index in 0..count {
        let section = optional + optional_size + index * 40;
        let start = u32_at(bytes, section + 12)?;
        let size = u32_at(bytes, section + 16)? as usize;
        if let Some(delta) = rva.checked_sub(start) {
            let delta = delta as usize;
            if delta.checked_add(length).is_some_and(|end| end <= size) {
                let offset = (u32_at(bytes, section + 20)? as usize)
                    .checked_add(delta)
                    .ok_or("offset overflow")?;
                return bytes
                    .get(offset..offset.checked_add(length).ok_or("offset overflow")?)
                    .ok_or("truncated section");
            }
        }
    }
    Err("virtual address is not backed by the requested file bytes")
}

pub const INTERNAL_ROM_VA: u32 = 0x42a800;
pub const PRG_LEN: usize = 0x8000;
pub const CHR_LEN: usize = 0x2000;

/// Address is specific to the analyzed executable; callers must verify the build.
pub fn internal_rom(exe: &[u8]) -> Result<&[u8], &'static str> {
    pe_slice(exe, INTERNAL_ROM_VA, PRG_LEN + CHR_LEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_wraps_like_x86() {
        assert_eq!(rom_checksum(&[]), 0);
        assert_eq!(rom_checksum(&[0, 1, 255]), 256);
        assert_eq!(rom_checksum(&vec![255; 16_843_010]), 254);
    }

    #[test]
    fn four_players_are_serialized_in_pairs() {
        let mut c = Controllers::default();
        for (i, mask) in [0x81, 0x42, 0x24, 0x18].into_iter().enumerate() {
            c.set_buttons(i, mask).unwrap();
        }
        c.write_strobe(0);
        for bit in 0..8 {
            assert_eq!(
                c.read_port(0).unwrap(),
                0xe0 | ((0x81 >> bit) & 1) | (((0x24 >> bit) & 1) << 1)
            );
            assert_eq!(
                c.read_port(1).unwrap(),
                0xe0 | ((0x42 >> bit) & 1) | (((0x18 >> bit) & 1) << 1)
            );
        }
        assert_eq!(c.read_port(0).unwrap(), 0xe0);
        c.write_strobe(0);
        assert_eq!(c.read_port(0).unwrap(), 0xe1);
    }

    #[test]
    fn high_strobe_reads_live_buttons_without_shifting() {
        let mut c = Controllers::default();
        c.write_strobe(1);
        c.set_buttons(2, 1).unwrap();
        assert_eq!(c.read_port(0).unwrap(), 0xe2);
        assert_eq!(c.read_port(0).unwrap(), 0xe2);
        c.clear_buttons(2, 1).unwrap();
        assert_eq!(c.read_port(0).unwrap(), 0xe0);
        assert!(c.read_port(2).is_err());
        assert!(c.set_buttons(4, 1).is_err());
    }

    #[test]
    fn malformed_images_return_errors() {
        for n in 0..512 {
            let mut bytes = vec![0; n];
            if n >= 2 {
                bytes[..2].copy_from_slice(b"MZ");
            }
            assert!(internal_rom(&bytes).is_err());
        }
    }
}
