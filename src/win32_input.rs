//! ConPTY's requested Win32 key protocol (DEC private mode 9001).
//! Terminal replies and paste remain VT bytes; keyboard events retain their
//! virtual key, Unicode character and modifier state instead of becoming text.
use egui::{Key, Modifiers};

#[derive(Default)]
pub struct Decoder {
    state: u8,
    csi: Vec<u8>,
    osc: bool,
    pub active: bool,
}
impl Decoder {
    pub fn advance(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            match self.state {
                0 => {
                    if byte == 27 {
                        self.state = 1;
                    }
                }
                1 => match byte {
                    b'[' => {
                        self.state = 2;
                        self.csi.clear();
                    }
                    b']' | b'P' | b'X' | b'^' | b'_' => {
                        self.osc = byte == b']';
                        self.state = 3;
                    }
                    b'c' => {
                        self.active = false;
                        self.state = 0;
                    }
                    27 => {}
                    _ => self.state = 0,
                },
                2 => {
                    if (0x40..=0x7e).contains(&byte) {
                        if matches!(byte, b'h' | b'l')
                            && self.csi.first() == Some(&b'?')
                            && self.csi[1..].split(|b| *b == b';').any(|n| {
                                std::str::from_utf8(n)
                                    .ok()
                                    .and_then(|n| n.parse::<u16>().ok())
                                    == Some(9001)
                            })
                        {
                            self.active = byte == b'h';
                        }
                        self.state = 0;
                    } else if byte == 27 {
                        self.state = 1;
                    } else if self.csi.len() < 128 {
                        self.csi.push(byte);
                    } else {
                        self.state = 0;
                    }
                }
                3 => {
                    if byte == 27 {
                        self.state = 4;
                    } else if byte == 7 && self.osc {
                        self.state = 0;
                    }
                }
                4 => {
                    if byte == b'\\' || (byte == 7 && self.osc) {
                        self.state = 0;
                    } else if byte != 27 {
                        self.state = 3;
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

fn record(vk: u16, unicode: u16, mods: Modifiers, enhanced: bool) -> Vec<u8> {
    let control = (if mods.ctrl { 8 } else { 0 })
        | (if mods.alt { 2 } else { 0 })
        | (if mods.shift { 16 } else { 0 })
        | (if enhanced { 256 } else { 0 });
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyW, MAPVK_VK_TO_VSC};
    let scan = if vk == 0 {
        0
    } else {
        unsafe { MapVirtualKeyW(vk.into(), MAPVK_VK_TO_VSC) }
    };
    format!("\x1b[{vk};{scan};{unicode};1;{control};1_\x1b[{vk};{scan};{unicode};0;{control};1_")
        .into_bytes()
}
pub fn text(text: &str, mods: Modifiers) -> Vec<u8> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::VkKeyScanW;
    text.encode_utf16()
        .flat_map(|c| {
            // VK_PACKET belongs to SendInput's Unicode injection API, rather
            // than the original keyboard event received by a console app.
            // Retain printable key identity; IME/unmapped Unicode has no VK.
            let mapped = if c < 128 {
                unsafe { VkKeyScanW(c) }
            } else {
                -1
            };
            let vk = if mapped == -1 {
                0
            } else {
                (mapped as u16) & 0xff
            };
            record(vk, c, mods, false)
        })
        .collect()
}
pub fn key(key: Key, mods: Modifiers) -> Option<Vec<u8>> {
    let (vk, unicode, enhanced) = match key {
        Key::Enter => (13, 13, false),
        Key::Tab => (9, 9, false),
        Key::Backspace => (8, 8, false),
        Key::Escape => (27, 27, false),
        Key::ArrowLeft => (37, 0, true),
        Key::ArrowUp => (38, 0, true),
        Key::ArrowRight => (39, 0, true),
        Key::ArrowDown => (40, 0, true),
        Key::Home => (36, 0, true),
        Key::End => (35, 0, true),
        Key::PageUp => (33, 0, true),
        Key::PageDown => (34, 0, true),
        Key::Insert => (45, 0, true),
        Key::Delete => (46, 0, true),
        Key::F1 => (112, 0, false),
        Key::F2 => (113, 0, false),
        Key::F3 => (114, 0, false),
        Key::F4 => (115, 0, false),
        Key::F5 => (116, 0, false),
        Key::F6 => (117, 0, false),
        Key::F7 => (118, 0, false),
        Key::F8 => (119, 0, false),
        Key::F9 => (120, 0, false),
        Key::F10 => (121, 0, false),
        Key::F11 => (122, 0, false),
        Key::F12 => (123, 0, false),
        Key::Space if mods.ctrl => (32, 0, false),
        _ if mods.ctrl => {
            let letter = match key {
                Key::A => 65,
                Key::B => 66,
                Key::C => 67,
                Key::D => 68,
                Key::E => 69,
                Key::F => 70,
                Key::G => 71,
                Key::H => 72,
                Key::I => 73,
                Key::J => 74,
                Key::K => 75,
                Key::L => 76,
                Key::M => 77,
                Key::N => 78,
                Key::O => 79,
                Key::P => 80,
                Key::Q => 81,
                Key::R => 82,
                Key::S => 83,
                Key::T => 84,
                Key::U => 85,
                Key::V => 86,
                Key::W => 87,
                Key::X => 88,
                Key::Y => 89,
                Key::Z => 90,
                _ => return None,
            };
            (letter, letter - 64, false)
        }
        _ => return None, // egui emits the actual character through Text/IME.
    };
    Some(record(vk, unicode, mods, enhanced))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requested_mode_survives_every_chunk_boundary_without_reading_osc_text() {
        let bytes = b"\x1b]0;title \x1b[?9001h\x07\x1b[?25;9001h";
        for split in 0..=bytes.len() {
            let mut parser = Decoder::default();
            parser.advance(&bytes[..split]);
            parser.advance(&bytes[split..]);
            assert!(parser.active);
            parser.advance(b"\x1b[?9001l");
            assert!(!parser.active);
            parser.advance(b"\x1b[?9001h\x1bc");
            assert!(!parser.active);
        }
        let mut parser = Decoder::default();
        parser.advance(b"\x1b]0;\x1b[?9001h\x07");
        assert!(!parser.active);
    }
    #[test]
    fn enter_navigation_controls_and_unicode_are_native_key_events() {
        let enter = String::from_utf8(key(Key::Enter, Modifiers::NONE).unwrap()).unwrap();
        assert!(enter.contains("[13;28;13;1;0;1_"));
        assert!(enter.contains("[13;28;13;0;0;1_"));
        let interrupt = String::from_utf8(key(Key::C, Modifiers::CTRL).unwrap()).unwrap();
        assert!(interrupt.contains(";3;1;8;1_"));
        let arrow = String::from_utf8(key(Key::ArrowDown, Modifiers::NONE).unwrap()).unwrap();
        assert!(arrow.contains(";0;1;256;1_"));
        assert!(key(Key::A, Modifiers::NONE).is_none());
        let unicode = String::from_utf8(text("한😀", Modifiers::NONE)).unwrap();
        assert_eq!(unicode.matches("[0;0;").count(), 6);
        let retry = String::from_utf8(text("r", Modifiers::NONE)).unwrap();
        assert!(retry.contains("[82;19;114;1;0;1_"));
    }
}
