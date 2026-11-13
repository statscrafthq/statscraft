//! Drawing of the Correspondence Analysis dialog.

use crate::app::{checkboxes, choice, field, target_one};
use crate::correspondence_dialog::{
    CA_DISTANCES, CA_NORMALIZATIONS, CA_PLOTS, CA_PRINT, Correspondence, Role, TableVariable,
};
use crate::dialogs::labelled_range;
use crate::stats_dialogs_view::small_field;
use eframe::egui::{self, ComboBox, ScrollArea, TextEdit, Ui};
use statscraft_data::{Dataset, Value};

pub fn correspondence(
    ui: &mut Ui,
    data: &Dataset,
    d: &mut Correspondence,
    picked: &mut Vec<String>,
) {
    table_variable(ui, data, "Row variable", &mut d.row, picked);
    table_variable(ui, data, "Column variable", &mut d.column, picked);
    ui.add_space(6.0);
    for (title, v) in [
        ("Row categories", &mut d.row),
        ("Column categories", &mut d.column),
    ] {
        egui::CollapsingHeader::new(title).show(ui, |ui| categories(ui, data, title, v));
    }
    egui::CollapsingHeader::new("Model").show(ui, |ui| {
        field(ui, "Dimensions in the solution", &mut d.dimensions, 40.0);
        choice(ui, "Distance", &CA_DISTANCES, &mut d.distance);
        ui.horizontal(|ui| {
            choice(
                ui,
                "Normalization",
                &CA_NORMALIZATIONS,
                &mut d.normalization,
            );
            let custom = d.normalization == CA_NORMALIZATIONS.len() - 1;
            small_field(ui, custom, &mut d.q);
        });
        ui.weak("A custom value runs from -1 (column principal) to 1 (row principal)");
    });
    egui::CollapsingHeader::new("Statistics").show(ui, |ui| {
        checkboxes(ui, &CA_PRINT, &mut d.print);
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.permutation, "Permuted tables, up to dimension");
            small_field(ui, d.permutation, &mut d.permutation_dims);
        });
    });
    egui::CollapsingHeader::new("Plots").show(ui, |ui| {
        let biplot = d.biplot_allowed();
        egui::Grid::new("ca plots").num_columns(2).show(ui, |ui| {
            for (i, (_, label)) in CA_PLOTS.iter().enumerate() {
                // shown off where the syntax leaves it out
                let on = i > 0 || biplot;
                let mut off = false;
                let flag = if on { &mut d.plots[i] } else { &mut off };
                ui.add_enabled(on, egui::Checkbox::new(flag, *label))
                    .on_disabled_hover_text("Not drawn with principal normalization");
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        field(
            ui,
            "Longest label, empty for full labels",
            &mut d.label_length,
            40.0,
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut d.ndim, "Only dimensions from");
            small_field(ui, d.ndim, &mut d.ndim_first);
            ui.label("to");
            small_field(ui, d.ndim, &mut d.ndim_last);
        });
        ui.weak("An empty last dimension plots up to the highest one");
    });
}

/// The variable with its range; a new variable starts from its lowest and highest labelled
/// value that is not user-missing, and with every category active. Leaving a changed range
/// field forgets the roles of the categories outside the range.
fn table_variable(
    ui: &mut Ui,
    data: &Dataset,
    title: &str,
    v: &mut TableVariable,
    picked: &mut Vec<String>,
) {
    let before = v.name.clone();
    target_one(ui, title, &mut v.name, picked);
    if v.name != before {
        v.roles.clear();
        (v.min, v.max) = labelled_range(data, v.name.trim());
    }
    // roles are forgotten once a field is left (or Enter pressed), not at each digit typed
    let left = ui.push_id(title, |ui| {
        ui.horizontal(|ui| {
            ui.label("Range from");
            let min = ui.add(TextEdit::singleline(&mut v.min).desired_width(50.0));
            ui.label("to");
            let max = ui.add(TextEdit::singleline(&mut v.max).desired_width(50.0));
            min.lost_focus() || max.lost_focus()
        })
        .inner
    });
    if left.inner {
        v.keep_roles_in_range();
    }
}

/// Each category of the range with its label and its role.
fn categories(ui: &mut Ui, data: &Dataset, id: &str, v: &mut TableVariable) {
    let Ok((min, max)) = v.range() else {
        ui.weak("Choose the variable and give its range first");
        return;
    };
    let labels = data
        .variable(&v.name)
        .map(|var| var.value_labels())
        .unwrap_or(&[]);
    let label = |value: i64| {
        labels
            .iter()
            .find(|(l, _)| *l == Value::Number(value as f64))
            .map_or("", |(_, text)| text.as_str())
    };
    let values: Vec<i64> = (min..=max).collect();
    // one group more than are in use, to start a new one
    let groups = v
        .roles
        .values()
        .filter_map(|r| match r {
            Role::Equal(g) => Some(*g),
            Role::Supplementary => None,
        })
        .max()
        .unwrap_or(0)
        + 1;
    let text = |role: Option<Role>| match role {
        None => "Active".to_string(),
        Some(Role::Supplementary) => "Supplementary".to_string(),
        Some(Role::Equal(g)) => format!("Equal group {g}"),
    };
    let height = ui.spacing().interact_size.y;
    ScrollArea::vertical()
        .id_salt(id)
        .max_height(220.0)
        .show_rows(ui, height, values.len(), |ui, rows| {
            for &value in &values[rows] {
                ui.horizontal(|ui| {
                    let current = v.roles.get(&value).copied();
                    let mut chosen = current;
                    ComboBox::from_id_salt((id, value))
                        .width(130.0)
                        .selected_text(text(current))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut chosen, None, text(None));
                            for role in std::iter::once(Role::Supplementary)
                                .chain((1..=groups).map(Role::Equal))
                            {
                                ui.selectable_value(&mut chosen, Some(role), text(Some(role)));
                            }
                        });
                    match label(value) {
                        "" => ui.label(value.to_string()),
                        l => ui.label(format!("{value}  {l}")),
                    };
                    if chosen != current {
                        match chosen {
                            Some(role) => v.roles.insert(value, role),
                            None => v.roles.remove(&value),
                        };
                    }
                });
            }
        });
}
