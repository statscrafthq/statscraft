//! The cell statistics CTABLES gives plugins: estimates, bases and the totals, weighted and not,
//! and their way through a saved output document.

use statscraft_engine::output_file::{self, Item};
use statscraft_engine::session::Session;
use statscraft_engine::{Border, Cell, CellStat, ColumnWidth, Decoration, Table, TableStyle};

/// Made-up answers: answer (1 yes, 2 no), group (1, 2), weight w and a score.
const DATA: &str = "DATA LIST LIST /answer group w score (4F4).
BEGIN DATA
1 1 2 1
1 1 1 2
2 1 1 3
1 2 1 4
2 2 1 5
2 2 1 6
2 2 3 7
END DATA.
";

fn tables(session: &mut Session, source: &str) -> Vec<Table> {
    let before = session.output_items("Document1").map_or(0, <[Item]>::len);
    session.run(source, None).expect("runs");
    let items = &session.output_items("Document1").unwrap()[before..];
    let leaves = output_file::leaves(items).into_iter();
    leaves.map(|(_, item)| item.shown().0).collect()
}

fn stat(table: &Table, row: usize, col: usize) -> &CellStat {
    let stats = table.cell_stats.as_ref().expect("cell statistics");
    let found = stats.cells.iter().find(|s| (s.row, s.col) == (row, col));
    found.unwrap_or_else(|| panic!("no statistic at {row}, {col}"))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

const TABLE: &str = "CTABLES /TABLE answer [C] [COLPCT] BY group [C]
  /CATEGORIES VARIABLES=group TOTAL=YES POSITION=BEFORE
  /CATEGORIES VARIABLES=answer TOTAL=YES.";

#[test]
fn a_crosstab_carries_estimates_bases_and_totals() {
    let mut session = Session::default();
    session.run(DATA, None).unwrap();
    // unweighted: (estimate, base) by column, total first, for the answer "yes"
    let table = tables(&mut session, TABLE).remove(0);
    let h = table.row_headers;
    let stats = table.cell_stats.as_ref().unwrap();
    assert_eq!(stats.total_columns, [h]);
    assert_eq!(stats.total_rows, [2]);
    for (col, estimate, base) in [
        (h, 3.0 / 7.0, 7.0),
        (h + 1, 2.0 / 3.0, 3.0),
        (h + 2, 0.25, 4.0),
    ] {
        let s = stat(&table, 0, col);
        assert_eq!(s.kind, "COLPCT.COUNT");
        assert!(close(s.estimate, estimate), "{s:?}");
        assert_eq!(
            (s.base, s.weighted_base, s.effective_base),
            (base, base, base)
        );
    }
    // the total row is 100% of its column
    assert!(close(stat(&table, 2, h + 1).estimate, 1.0));

    // weighted: the base stays the cases, the effective base is (sum w)^2 / sum w^2
    let table = tables(&mut session, &format!("WEIGHT BY w.\n{TABLE}")).remove(0);
    for (col, estimate, base, weighted, effective) in [
        (h, 0.4, 7.0, 10.0, 100.0 / 18.0),
        (h + 1, 0.75, 3.0, 4.0, 16.0 / 6.0),
        (h + 2, 1.0 / 6.0, 4.0, 6.0, 3.0),
    ] {
        let s = stat(&table, 0, col);
        assert!(close(s.estimate, estimate), "{s:?}");
        assert_eq!((s.base, s.weighted_base), (base, weighted));
        assert!(close(s.effective_base, effective), "{s:?}");
    }

    // a mean carries its standard deviation, and other procedures carry nothing
    session.run("WEIGHT OFF.", None).unwrap();
    let table = tables(
        &mut session,
        "CTABLES /TABLE score [S] [MEAN] BY group [C].",
    )
    .remove(0);
    let s = stat(&table, 0, table.row_headers);
    assert_eq!(
        (s.kind.as_str(), s.estimate, s.base, s.sd),
        ("MEAN", 2.0, 3.0, Some(1.0))
    );
    let frequencies = tables(&mut session, "FREQUENCIES group.");
    assert!(frequencies.iter().all(|t| t.cell_stats.is_none()));

    // letters merged into the table move the columns, and the statistics move with them
    let merged = "CTABLES /TABLE answer [C] [COLPCT] BY group [C]
  /COMPARETEST TYPE=PROP MERGE=YES STYLE=SIMPLE.";
    let table = tables(&mut session, merged).remove(0);
    let stats = table.cell_stats.as_ref().unwrap();
    assert_eq!(stats.cells.len(), 4);
    for s in &stats.cells {
        let Cell::Number(shown) = table.rows[s.row][s.col] else {
            panic!("{s:?} is not on a number");
        };
        assert!(close(shown / 100.0, s.estimate), "{s:?} on {shown}");
    }
}

#[test]
fn statistics_and_decorations_survive_the_output_file() {
    let mut session = Session::default();
    session.run(DATA, None).unwrap();
    session.run(TABLE, None).unwrap();
    let items = session.output_items("Document1").unwrap().to_vec();
    let mut table = items_table(&items);
    let h = table.row_headers;
    let cells = vec![
        Decoration {
            row: 0,
            col: h + 1,
            fill: Some("#a9ecf5".into()),
            text: Some("#094e56".into()),
            bold: true,
            border: Some(Border {
                bottom: Some("#4bacc6".into()),
                left: Some("blue".into()),
                ..Border::default()
            }),
            align: Some("center".into()),
            comment: Some("66.7% (13.3% to 100%)".into()),
            ..Decoration::default()
        },
        // off the table, or with a colour that is not #rrggbb: dropped or cleaned
        Decoration {
            row: 99,
            col: 0,
            ..Decoration::default()
        },
        Decoration {
            row: 1,
            col: h,
            fill: Some("red\" onclick=\"x".into()),
            align: Some("middle".into()),
            comment: Some("total".into()),
            ..Decoration::default()
        },
    ];
    let header = |row, col| Decoration {
        row,
        col,
        italic: true,
        comment: Some("no comments in the header".into()),
        ..Decoration::default()
    };
    let style = TableStyle {
        font: Some("Cambria".into()),
        font_size: Some(8.0),
        // the second is below the header lines
        header_cells: vec![header(0, h), header(9, h)],
        plain: true,
        row_labels: Some("stacked".into()),
        // off the table, too wide, and the same column twice: dropped, clamped, the first kept
        column_widths: [(99, 5.0), (h, 3.0), (0, 400.0), (h, 9.0)]
            .map(|(col, width)| ColumnWidth { col, width })
            .to_vec(),
    };
    table.set_decorations(cells, style);
    assert_eq!(table.decorations.len(), 2);
    assert_eq!(table.decoration(1, h).unwrap().fill, None);
    let border = table.decoration(0, h + 1).unwrap().border.clone().unwrap();
    assert_eq!(
        (border.bottom.as_deref(), border.left),
        (Some("#4bacc6"), None)
    );
    assert_eq!(table.style.header_cells.len(), 1);
    assert_eq!(table.header_decoration(0, h).unwrap().comment, None);
    assert_eq!(table.decoration(1, h).unwrap().align, None);
    assert!(table.stacked() && table.style.plain);
    let widths = [(0, 255.0), (h, 3.0)].map(|(col, width)| ColumnWidth { col, width });
    assert_eq!(table.style.column_widths, widths);
    // a font name that could break out of a style, and a size beyond Excel's, are dropped
    let mut odd = table.clone();
    let quoted = TableStyle {
        font: Some("x';background:red".into()),
        font_size: Some(1000.0),
        row_labels: Some("nested".into()),
        ..TableStyle::default()
    };
    odd.set_decorations(Vec::new(), quoted);
    assert!(odd.style.is_empty());
    let text = output_file::write(&[Item::Table(std::sync::Arc::new(table.clone()))]);
    let read = items_table(&output_file::read(&text).unwrap());
    assert_eq!(read.cell_stats, table.cell_stats);
    assert_eq!(read.decorations, table.decorations);
    assert_eq!(read.style, table.style);
    // and in the JSON of a run
    let json = serde_json::to_value(&table).unwrap();
    assert_eq!(json["cell_stats"]["total_columns"][0], h);
    assert_eq!(json["decorations"][0]["fill"], "#a9ecf5");
    assert_eq!(json["decorations"][0]["bold"], true);
    assert_eq!(json["style"]["font"], "Cambria");
    assert_eq!(json["style"]["row_labels"], "stacked");
    assert_eq!(json["style"]["column_widths"][1]["width"], 3.0);
    assert_eq!(json["decorations"][0]["align"], "center");
    let html = statscraft_engine::cell_attributes(&table, 0, h + 1);
    assert_eq!(
        html,
        " style=\"background:#a9ecf5;color:#094e56;font-weight:bold;text-align:center;\
         border-bottom:1px solid #4bacc6;\" title=\"66.7% (13.3% to 100%)\""
    );
    assert_eq!(
        statscraft_engine::table_attributes(&table),
        " style=\"font-family:'Cambria';font-size:8pt;\""
    );
    assert_eq!(
        statscraft_engine::header_attributes(&table, 0, h, 1),
        " style=\"font-style:italic;\""
    );
}

fn items_table(items: &[Item]) -> Table {
    let leaves = output_file::leaves(items);
    leaves
        .into_iter()
        .map(|(_, item)| item.shown().0)
        .next()
        .unwrap()
}
