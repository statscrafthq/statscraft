//! Drawing of the Response Rate dialog.

use crate::app::{choice, field, target_one};
use crate::response_rate_dialog::{MAX_COUNTS, MIN_RATES, ResponseRate};
use eframe::egui::Ui;

pub fn response_rate(ui: &mut Ui, d: &mut ResponseRate, picked: &mut Vec<String>) {
    target_one(ui, "Numerator (responses)", &mut d.numerator, picked);
    target_one(ui, "Denominator (letters sent)", &mut d.denominator, picked);
    target_one(ui, "Group", &mut d.group, picked);
    ui.add_space(6.0);
    ui.strong("Limits");
    choice(ui, "Lowest rate", &MIN_RATES, &mut d.min_rate);
    match d.min_rate {
        1 => field(ui, "Rate %", &mut d.rate, 60.0),
        2 => {
            field(ui, "Cost of a letter", &mut d.cost, 60.0);
            field(ui, "Revenue of a response", &mut d.revenue, 60.0);
        }
        _ => {}
    }
    choice(ui, "Highest count", &MAX_COUNTS, &mut d.max_count);
    match d.max_count {
        1 => field(ui, "Count", &mut d.count, 70.0),
        2 => field(ui, "Percent of the total", &mut d.percent, 60.0),
        _ => {}
    }
}
