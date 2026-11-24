//! POWER against R on study plans other than those of `power.rs`: the t-test powers and sizes
//! against `power.t.test(strict = TRUE)` (the strict form counts both tails of a two-sided
//! test, as the manual's formula does; R's default leaves the far tail out and is smaller by
//! that tail), the one-way ANOVA power against `power.anova.test` and the noncentral F, and
//! the two-proportion normal approximation against the manual's formula written out in R
//! (`power.prop.test` uses other variances and one tail, so it is not the same quantity).

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};

fn power(source: &str) -> Table {
    let result = Session::default()
        .run(source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let command = result.commands.into_iter().last().unwrap();
    command
        .tables
        .into_iter()
        .find(|t| t.title == "Power Analysis")
        .unwrap_or_else(|| panic!("no table: {source}"))
}

fn at(table: &Table, column: &str) -> f64 {
    let i = table
        .columns
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("no column {column} in {:?}", table.columns));
    match &table.rows[0][i] {
        Cell::Number(n) => *n,
        other => panic!("{other:?} is no number"),
    }
}

fn near(got: f64, want: f64, tolerance: f64, what: &str) {
    assert!(
        (got - want).abs() <= tolerance * want.abs(),
        "{what}: {got} vs {want}"
    );
}

#[test]
fn the_t_test_powers_and_sizes_agree_with_power_t_test() {
    let t = power("POWER MEANS ONESAMPLE /PARAMETERS N=17 SD=2.5 MEAN=1.1 SIGNIFICANCE=0.01.");
    near(at(&t, "Power"), 0.172507485556, 1e-9, "one sample");
    // 67.62 rounds up to 68
    let t = power("POWER MEANS ONESAMPLE /PARAMETERS POWER=0.9 SD=3 MEAN=1.2.");
    assert_eq!(at(&t, "N"), 68.0);
    near(
        at(&t, "Actual Power"),
        0.901628034759,
        1e-9,
        "one sample size",
    );

    let t = power("POWER MEANS INDEPENDENT /PARAMETERS N=22 22 SD=3.2 MEAN=2 SIGNIFICANCE=0.1.");
    near(at(&t, "Power"), 0.65349485026, 1e-9, "independent");
    // 54.78 rounds up to 55 a group
    let t = power("POWER MEANS INDEPENDENT /PARAMETERS POWER=0.9 SD=3.2 MEAN=2.");
    assert_eq!((at(&t, "N1"), at(&t, "N2")), (55.0, 55.0));
    near(
        at(&t, "Actual Power"),
        0.901176196605,
        1e-9,
        "independent size",
    );

    // paired: the sd of the differences is 3 * sqrt(2 * (1 - 0.5)) = 3
    let t = power("POWER MEANS RELATED /PARAMETERS NPAIRS=19 SD=3 3 RHO=0.5 MEAN=1.4.");
    near(at(&t, "Power"), 0.486212226996, 1e-9, "paired");
    // 43.19 rounds up to 44 pairs
    let t = power("POWER MEANS RELATED /PARAMETERS POWER=0.85 SD=3 3 RHO=0.5 MEAN=1.4.");
    assert_eq!(at(&t, "N Pairs"), 44.0);
    near(at(&t, "Actual Power"), 0.856726575923, 1e-9, "paired size");
}

#[test]
fn the_oneway_anova_power_agrees_with_the_noncentral_f() {
    // three groups of 8, means 10 12 15, sd 4: power.anova.test(between.var = var(means), ...)
    let t =
        power("POWER ONEWAY ANOVA /PARAMETERS GROUP_SIZES=8 8 8 GROUP_MEANS=10 12 15 POOLED_SD=4.");
    near(at(&t, "Power"), 0.543571738, 1e-8, "three groups");
    // four groups of 15 at the 0.01 level
    let t = power(
        "POWER ONEWAY ANOVA /PARAMETERS GROUP_SIZES=15 15 15 15 GROUP_MEANS=10 11 12 15 \
         POOLED_SD=5 SIGNIFICANCE=0.01.",
    );
    near(at(&t, "Power"), 0.390012121, 1e-8, "four groups");
}

#[test]
fn the_two_proportion_normal_power_follows_the_manuals_formula() {
    // pooled standard error under the null, both tails: 0.44552739988 in R
    let t = power("POWER PROPORTIONS INDEPENDENT /PARAMETERS N=40 40 PROPORTIONS=0.3 0.5.");
    near(at(&t, "Power"), 0.44552739988, 1e-9, "pooled");
    // directional, one tail
    let t = power(
        "POWER PROPORTIONS INDEPENDENT /PARAMETERS N=40 40 PROPORTIONS=0.3 0.5 TEST=DIRECTIONAL.",
    );
    near(at(&t, "Power"), 0.573298710483, 1e-9, "directional");
    // unpooled with the continuity correction c = 0.5 (1/n1 + 1/n2)
    let t = power(
        "POWER PROPORTIONS INDEPENDENT /PARAMETERS N=40 40 PROPORTIONS=0.3 0.5 CONTINUITY=TRUE \
         POOLED=FALSE.",
    );
    near(at(&t, "Power"), 0.371450109046, 1e-9, "corrected");
}
