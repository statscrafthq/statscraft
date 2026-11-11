//! The brand's accent in the window: petrol for selection, focus, links and the button that
//! confirms a dialog, in the light or dark value as the visuals are. The rest stays egui's.

use eframe::egui::{self, Color32, RichText, Visuals};

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// Petrol and the text that stands on it, light and dark.
const PETROL: [Color32; 2] = [hex(0x0B4F5C), hex(0x5CC2D2)];
const ON_PETROL: [Color32; 2] = [hex(0xFFFFFF), hex(0x0C1517)];
/// Selected text and cells: the lighter petrol of the charts halfway to the paper, so text in
/// ink stays readable on it (light: #6FA7B2 with white, dark: #2D6E79 with #0C1517).
const SELECTION: [Color32; 2] = [hex(0xB7D3D8), hex(0x1C4148)];
/// Petrol on the selection; the dark mode takes the stronger petrol for contrast.
const ON_SELECTION: [Color32; 2] = [hex(0x0B4F5C), hex(0x8AD6E2)];

fn mode(visuals: &Visuals) -> usize {
    usize::from(visuals.dark_mode)
}

/// Petrol for selection, focus outlines and links.
pub fn accent(visuals: &mut Visuals) {
    let m = mode(visuals);
    visuals.selection.bg_fill = SELECTION[m];
    visuals.selection.stroke.color = ON_SELECTION[m];
    visuals.hyperlink_color = PETROL[m];
}

/// The button that confirms a dialog, filled with petrol.
pub fn primary(ui: &egui::Ui, text: &str) -> egui::Button<'static> {
    let m = mode(ui.visuals());
    egui::Button::new(RichText::new(text).color(ON_PETROL[m])).fill(PETROL[m])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG contrast ratio of two colours.
    fn contrast(a: Color32, b: Color32) -> f32 {
        let lum = |c: Color32| {
            let f = |v: u8| {
                let v = f32::from(v) / 255.0;
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn text_on_the_accent_meets_aa_in_both_modes() {
        for m in 0..2 {
            let ink = [hex(0x12191C), hex(0xE4EDEF)][m];
            assert!(contrast(ON_PETROL[m], PETROL[m]) >= 4.5, "button {m}");
            assert!(contrast(ink, SELECTION[m]) >= 4.5, "selected text {m}");
            assert!(
                contrast(ON_SELECTION[m], SELECTION[m]) >= 4.5,
                "selected label {m}"
            );
        }
        let mut dark = Visuals::dark();
        accent(&mut dark);
        assert_eq!(dark.hyperlink_color, PETROL[1]);
    }
}
