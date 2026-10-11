use alacritty_terminal::{
    term::color::Colors,
    vte::ansi::{Color, NamedColor, Rgb},
};
use std::sync::LazyLock;

pub fn rgb(index: usize, light: bool) -> Rgb {
    const DARK: [[u8; 3]; 16] = [
        [25, 31, 42],
        [235, 111, 116],
        [130, 202, 157],
        [230, 189, 119],
        [125, 169, 230],
        [186, 149, 222],
        [106, 218, 187],
        [213, 220, 231],
        [88, 101, 120],
        [245, 140, 145],
        [159, 223, 180],
        [248, 210, 143],
        [154, 193, 245],
        [208, 179, 239],
        [148, 232, 211],
        [246, 248, 252],
    ];
    const LIGHT: [[u8; 3]; 16] = [
        [32, 41, 56],
        [166, 32, 53],
        [24, 112, 59],
        [134, 89, 0],
        [37, 82, 166],
        [121, 53, 146],
        [5, 107, 112],
        [229, 233, 240],
        [90, 105, 122],
        [185, 32, 63],
        [18, 121, 65],
        [147, 97, 0],
        [37, 99, 185],
        [139, 57, 162],
        [0, 119, 120],
        [246, 248, 252],
    ];
    let [r, g, b] = match index {
        0..=15 => {
            if light {
                LIGHT[index]
            } else {
                DARK[index]
            }
        }
        16..=231 => {
            let index = index - 16;
            let component = |c: usize| if c == 0 { 0 } else { (55 + c * 40) as u8 };
            [
                component(index / 36),
                component(index / 6 % 6),
                component(index % 6),
            ]
        }
        232..=255 => {
            let value = (8 + (index - 232) * 10) as u8;
            [value, value, value]
        }
        257 => {
            if light {
                [245, 247, 250]
            } else {
                [17, 21, 29]
            }
        }
        258 => {
            if light {
                LIGHT[6]
            } else {
                DARK[6]
            }
        }
        // Dim named colors retain their hues; the renderer applies opacity for SGR dim.
        259..=266 => {
            if light {
                LIGHT[index - 259]
            } else {
                DARK[index - 259]
            }
        }
        _ => {
            if light {
                [38, 47, 60]
            } else {
                [213, 220, 231]
            }
        }
    };
    Rgb { r, g, b }
}

pub fn resolve(color: Color, overrides: &Colors, light: bool) -> Rgb {
    match color {
        Color::Spec(rgb) => rgb,
        Color::Indexed(index) => {
            overrides[index as usize].unwrap_or_else(|| rgb(index as usize, light))
        }
        Color::Named(named) => overrides[named].unwrap_or_else(|| rgb(named as usize, light)),
    }
}

/// Resolve the original SGR pair before applying inverse video. Explicit RGB/OSC
/// colors and explicit foreground/background pairs retain the program's colors.
pub fn cell_colors(
    foreground: Color,
    background: Color,
    inverse: bool,
    overrides: &Colors,
    light: bool,
) -> (Rgb, Rgb) {
    let mut fg = resolve(foreground, overrides, light);
    let bg = resolve(background, overrides, light);
    let index = match foreground {
        Color::Spec(_) => None,
        Color::Indexed(index) => Some(index as usize),
        Color::Named(named) => Some(named as usize),
    };
    if let Some(index) = index.filter(|index| overrides[*index].is_none()) {
        let default_foreground = matches!(index, 256 | 267 | 268);
        let white_on_default_background =
            matches!(index, 7 | 15 | 266) && background == Color::Named(NamedColor::Background);
        if (default_foreground || white_on_default_background) && contrast(fg, bg) < 4.5 {
            let dark = rgb(256, true);
            let bright = rgb(15, false);
            let alternative = if contrast(dark, bg) >= contrast(bright, bg) {
                dark
            } else {
                bright
            };
            if contrast(alternative, bg) > contrast(fg, bg) {
                fg = alternative;
            }
        }
    }
    if inverse {
        (bg, fg)
    } else {
        (fg, bg)
    }
}

fn luminance(rgb: Rgb) -> f64 {
    // Shared lookup avoids gamma conversions for each visible terminal cell.
    static LINEAR: LazyLock<[f64; 256]> = LazyLock::new(|| {
        std::array::from_fn(|value| {
            let value = value as f64 / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        })
    });
    0.2126 * LINEAR[rgb.r as usize]
        + 0.7152 * LINEAR[rgb.g as usize]
        + 0.0722 * LINEAR[rgb.b as usize]
}

fn contrast(a: Rgb, b: Rgb) -> f64 {
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn light_theme_default_text_and_palette_remain_readable() {
        let colors = Colors::default();
        for index in (0..16).chain([256, 258]) {
            let foreground = if index < 256 {
                Color::Indexed(index as u8)
            } else if index == 256 {
                Color::Named(NamedColor::Foreground)
            } else {
                Color::Named(NamedColor::Cursor)
            };
            let (fg, bg) = cell_colors(
                foreground,
                Color::Named(NamedColor::Background),
                false,
                &colors,
                true,
            );
            assert!(
                contrast(fg, bg) >= 4.5,
                "color {index}: contrast {}",
                contrast(fg, bg)
            );
        }
    }

    #[test]
    fn explicit_black_white_pairs_and_default_inverse_are_readable_in_both_themes() {
        let colors = Colors::default();
        for light in [false, true] {
            for (foreground, background) in [
                (NamedColor::Black, NamedColor::White),
                (NamedColor::White, NamedColor::Black),
                (NamedColor::Black, NamedColor::BrightWhite),
                (NamedColor::BrightWhite, NamedColor::Black),
                (NamedColor::Foreground, NamedColor::Background),
            ] {
                let pair = cell_colors(
                    Color::Named(foreground),
                    Color::Named(background),
                    false,
                    &colors,
                    light,
                );
                assert!(
                    contrast(pair.0, pair.1) >= 4.5,
                    "{foreground:?}/{background:?} light={light}"
                );
                assert_eq!(
                    cell_colors(
                        Color::Named(foreground),
                        Color::Named(background),
                        true,
                        &colors,
                        light
                    ),
                    (pair.1, pair.0)
                );
                if (foreground as usize) < 16 && (background as usize) < 16 {
                    assert_eq!(
                        cell_colors(
                            Color::Indexed(foreground as u8),
                            Color::Indexed(background as u8),
                            false,
                            &colors,
                            light
                        ),
                        pair
                    );
                }
            }
        }
    }

    #[test]
    fn default_text_remains_readable_on_explicit_ansi_backgrounds() {
        let colors = Colors::default();
        for light in [false, true] {
            for index in 0..16 {
                let (fg, bg) = cell_colors(
                    Color::Named(NamedColor::Foreground),
                    Color::Indexed(index),
                    false,
                    &colors,
                    light,
                );
                assert_eq!(bg, rgb(index as usize, light));
                assert!(
                    contrast(fg, bg) >= 4.5,
                    "background {index} light={light}: {}",
                    contrast(fg, bg)
                );
            }
        }
    }

    #[test]
    fn explicit_colors_and_palette_overrides_keep_their_exact_values() {
        let custom = Rgb {
            r: 245,
            g: 232,
            b: 201,
        };
        let mut colors = Colors::default();
        for light in [false, true] {
            // Matching colors can be intentional; do not reinterpret application RGB.
            assert_eq!(
                cell_colors(
                    Color::Spec(custom),
                    Color::Spec(custom),
                    false,
                    &colors,
                    light
                ),
                (custom, custom)
            );
            assert_eq!(
                cell_colors(Color::Indexed(7), Color::Indexed(7), false, &colors, light),
                (rgb(7, light), rgb(7, light))
            );
        }
        colors[NamedColor::White] = Some(custom);
        colors[NamedColor::Foreground] = Some(custom);
        colors[NamedColor::Background] = Some(custom);
        for foreground in [
            Color::Indexed(7),
            Color::Named(NamedColor::White),
            Color::Named(NamedColor::Foreground),
        ] {
            assert_eq!(
                cell_colors(
                    foreground,
                    Color::Named(NamedColor::Background),
                    false,
                    &colors,
                    true
                ),
                (custom, custom)
            );
            assert_eq!(
                cell_colors(
                    foreground,
                    Color::Named(NamedColor::Black),
                    true,
                    &colors,
                    true
                ),
                (rgb(0, true), custom)
            );
        }
        colors[NamedColor::White] = None;
        let (fg, bg) = cell_colors(
            Color::Named(NamedColor::Black),
            Color::Indexed(7),
            false,
            &colors,
            true,
        );
        assert_eq!((fg, bg), (rgb(0, true), rgb(7, true)));
        colors[7] = Some(custom);
        assert_eq!(
            cell_colors(
                Color::Named(NamedColor::Black),
                Color::Indexed(7),
                false,
                &colors,
                true
            ),
            (fg, custom)
        );
    }
}
