//! The colour themes of the syntax editor: ready-made palettes of code editors, each with its
//! background, text and the colours of the kinds of words. The palettes and their licences are
//! named in `NOTICES`, which Help, About shows.

use crate::highlight::Kind;
use eframe::egui::Color32;

pub struct Theme {
    pub name: &'static str,
    pub dark: bool,
    pub background: Color32,
    pub text: Color32,
    command: Color32,
    subcommand: Color32,
    keyword: Color32,
    string: Color32,
    number: Color32,
    comment: Color32,
    macro_word: Color32,
    period: Color32,
}

impl Theme {
    pub fn color(&self, kind: Kind) -> Color32 {
        match kind {
            Kind::Plain => self.text,
            Kind::Command => self.command,
            Kind::Subcommand => self.subcommand,
            Kind::Keyword => self.keyword,
            Kind::String => self.string,
            Kind::Number => self.number,
            Kind::Comment => self.comment,
            Kind::Macro => self.macro_word,
            Kind::Period => self.period,
        }
    }
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// Background, text, command, subcommand, keyword, string, number, comment, macro, period.
#[allow(clippy::too_many_arguments)]
const fn theme(name: &'static str, dark: bool, colors: [u32; 10]) -> Theme {
    Theme {
        name,
        dark,
        background: hex(colors[0]),
        text: hex(colors[1]),
        command: hex(colors[2]),
        subcommand: hex(colors[3]),
        keyword: hex(colors[4]),
        string: hex(colors[5]),
        number: hex(colors[6]),
        comment: hex(colors[7]),
        macro_word: hex(colors[8]),
        period: hex(colors[9]),
    }
}

/// The themes in the order View offers them; the first is the default.
pub const THEMES: [Theme; 7] = [
    // base, text, blue, teal, mauve, green, peach, overlay 1, yellow, red
    theme(
        "Catppuccin Latte",
        false,
        [
            0xeff1f5, 0x4c4f69, 0x1e66f5, 0x179299, 0x8839ef, 0x40a02b, 0xfe640b, 0x8c8fa1,
            0xdf8e1d, 0xd20f39,
        ],
    ),
    theme(
        "Catppuccin Mocha",
        true,
        [
            0x1e1e2e, 0xcdd6f4, 0x89b4fa, 0x94e2d5, 0xcba6f7, 0xa6e3a1, 0xfab387, 0x7f849c,
            0xf9e2af, 0xf38ba8,
        ],
    ),
    // background, foreground, blue, cyan, purple, green, orange, comment, yellow, red
    theme(
        "One Dark",
        true,
        [
            0x282c34, 0xabb2bf, 0x61afef, 0x56b6c2, 0xc678dd, 0x98c379, 0xd19a66, 0x5c6370,
            0xe5c07b, 0xe06c75,
        ],
    ),
    // base3 or base03, base00 or base0, blue, violet, green, cyan, magenta, base1 or base01,
    // yellow, red
    theme(
        "Solarized Light",
        false,
        [
            0xfdf6e3, 0x657b83, 0x268bd2, 0x6c71c4, 0x859900, 0x2aa198, 0xd33682, 0x93a1a1,
            0xb58900, 0xdc322f,
        ],
    ),
    theme(
        "Solarized Dark",
        true,
        [
            0x002b36, 0x839496, 0x268bd2, 0x6c71c4, 0x859900, 0x2aa198, 0xd33682, 0x586e75,
            0xb58900, 0xdc322f,
        ],
    ),
    // canvas, text, keyword, entity, constant, string, variable, comment, markup, keyword
    theme(
        "GitHub Light",
        false,
        [
            0xffffff, 0x1f2328, 0xcf222e, 0x6639ba, 0x0550ae, 0x0a3069, 0x953800, 0x59636e,
            0x116329, 0xcf222e,
        ],
    ),
    theme(
        "GitHub Dark",
        true,
        [
            0x0d1117, 0xf0f6fc, 0xff7b72, 0xd2a8ff, 0x79c0ff, 0xa5d6ff, 0xffa657, 0x9198a1,
            0x7ee787, 0xff7b72,
        ],
    ),
];

/// The theme of that name, or the default.
pub fn by_name(name: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.name == name).unwrap_or(&THEMES[0])
}

/// Where the palettes come from, with the notices their MIT licences ask for.
pub const NOTICES: &str = "\
Catppuccin Latte and Mocha: the Catppuccin palette, Copyright (c) 2021 Catppuccin, MIT License.
One Dark: the One Dark syntax theme, Copyright (c) 2016 GitHub Inc., MIT License.
Solarized Light and Dark: Copyright (c) 2011 Ethan Schoonover, MIT License.
GitHub Light and Dark: the Primer colours, Copyright (c) 2018 GitHub Inc., MIT License.

Each is used under the MIT License: Permission is hereby granted, free of charge, to any person
obtaining a copy of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights to use, copy,
modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so, subject to the following
conditions: The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software. THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT
WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE
OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_has_a_name_and_readable_words() {
        assert_eq!(by_name("One Dark").name, "One Dark");
        assert_eq!(by_name("no such").name, THEMES[0].name);
        for theme in &THEMES {
            assert!(NOTICES.contains(theme.name.split(' ').next().unwrap()));
            // dark themes have a dark background and light text, light ones the other way
            let light = |c: Color32| u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b()) > 382;
            assert_eq!(light(theme.background), !theme.dark, "{}", theme.name);
            assert_eq!(light(theme.text), theme.dark, "{}", theme.name);
            assert_eq!(theme.color(Kind::Plain), theme.text);
        }
    }
}
