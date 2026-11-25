//! Drawing of the Response Rate dialog.

use crate::app::{choice, field, target_one};
use crate::i18n::t;
use crate::response_rate_dialog::{MAX_COUNTS, MIN_RATES, ResponseRate};
use eframe::egui::Ui;

pub fn response_rate(ui: &mut Ui, d: &mut ResponseRate, picked: &mut Vec<String>) {
    target_one(ui, t("stats-resprate-numerator"), &mut d.numerator, picked);
    target_one(
        ui,
        t("stats-resprate-denominator"),
        &mut d.denominator,
        picked,
    );
    target_one(ui, t("stats-resprate-group"), &mut d.group, picked);
    ui.add_space(6.0);
    ui.strong(t("stats-resprate-limits"));
    choice(
        ui,
        t("stats-resprate-lowest-rate"),
        &MIN_RATES,
        &mut d.min_rate,
    );
    match d.min_rate {
        1 => field(ui, t("stats-resprate-rate"), &mut d.rate, 60.0),
        2 => {
            field(ui, t("stats-resprate-cost"), &mut d.cost, 60.0);
            field(ui, t("stats-resprate-revenue"), &mut d.revenue, 60.0);
        }
        _ => {}
    }
    choice(
        ui,
        t("stats-resprate-highest-count"),
        &MAX_COUNTS,
        &mut d.max_count,
    );
    match d.max_count {
        1 => field(ui, t("stats-resprate-count"), &mut d.count, 70.0),
        2 => field(ui, t("stats-resprate-percent"), &mut d.percent, 60.0),
        _ => {}
    }
}
