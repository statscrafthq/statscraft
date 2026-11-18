//! The Response Rate dialog (RESPONSE RATE): response rates of a numerator over a denominator
//! in each group, with cumulative rates and limits. Limits left off add nothing to the syntax.

use crate::stats_dialogs::{numeric, parse};
use crate::syntax_text::{names, number};
use statscraft_data::Dataset;

/// What /MINRATE sets, the command's own choice first.
pub const MIN_RATES: [(&str, &str); 3] = [
    ("NONE", "No lowest rate"),
    ("RATE", "A lowest cumulative rate in percent"),
    ("FORMULA", "From the cost and the revenue of a response"),
];

/// What /MAXCOUNT sets, the command's own choice first.
pub const MAX_COUNTS: [(&str, &str); 3] = [
    ("ALL", "No highest count"),
    ("COUNT", "A highest cumulative count"),
    (
        "PERCENT",
        "A highest cumulative count as a percentage of the total",
    ),
];

#[derive(Clone, PartialEq, Debug, Default)]
pub struct ResponseRate {
    pub numerator: String,
    pub denominator: String,
    pub group: String,
    /// Index into `MIN_RATES`, with the value it needs.
    pub min_rate: usize,
    pub rate: String,
    pub cost: String,
    pub revenue: String,
    /// Index into `MAX_COUNTS`, with the value it needs.
    pub max_count: usize,
    pub count: String,
    pub percent: String,
}

impl ResponseRate {
    pub fn syntax(&self, data: &Dataset) -> Result<String, String> {
        numeric(data, [&self.numerator, &self.denominator])?;
        for (value, what) in [
            (&self.numerator, "numerator"),
            (&self.denominator, "denominator"),
            (&self.group, "grouping variable"),
        ] {
            if value.trim().is_empty() {
                return Err(format!("set the {what}"));
            }
        }
        let list = [
            self.numerator.clone(),
            self.denominator.clone(),
            self.group.clone(),
        ];
        names(&list, "Variables")?;
        let mut text = format!(
            "RESPONSE RATE /VARIABLES NUMERATOR={} DENOMINATOR={} GROUP={}",
            self.numerator.trim(),
            self.denominator.trim(),
            self.group.trim()
        );
        match self.min_rate {
            1 => {
                let rate = parse(&self.rate, "the lowest rate")?;
                if rate <= 0.0 || rate >= 100.0 {
                    return Err("the lowest rate must be above 0 and below 100".into());
                }
                text.push_str(&format!("\n  /MINRATE RATE={}", number(rate)));
            }
            2 => {
                let cost = parse(&self.cost, "the cost")?;
                let revenue = parse(&self.revenue, "the revenue")?;
                if cost <= 0.0 || revenue <= 0.0 || cost >= revenue {
                    return Err(
                        "cost and revenue must be above 0, with the cost below the revenue".into(),
                    );
                }
                text.push_str(&format!(
                    "\n  /MINRATE FORMULA={}, {}",
                    number(cost),
                    number(revenue)
                ));
            }
            _ => {}
        }
        match self.max_count {
            1 => {
                let n = parse(&self.count, "the highest count")?;
                if n < 1.0 || n.fract() != 0.0 {
                    return Err("the highest count must be a positive whole number".into());
                }
                text.push_str(&format!("\n  /MAXCOUNT COUNT={}", number(n)));
            }
            2 => {
                let p = parse(&self.percent, "the percentage")?;
                if p <= 0.0 || p >= 100.0 {
                    return Err("the percentage must be above 0 and below 100".into());
                }
                text.push_str(&format!("\n  /MAXCOUNT PERCENT={}", number(p)));
            }
            _ => {}
        }
        text.push('.');
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testing::open;

    fn base() -> ResponseRate {
        ResponseRate {
            numerator: "q1_satisfaction".into(),
            denominator: "age".into(),
            group: "region".into(),
            ..Default::default()
        }
    }

    #[test]
    fn the_least_and_the_most_the_dialog_writes() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        assert_eq!(
            base().syntax(&data).unwrap(),
            "RESPONSE RATE /VARIABLES NUMERATOR=q1_satisfaction DENOMINATOR=age GROUP=region."
        );
        let d = ResponseRate {
            min_rate: 2,
            cost: "0.6".into(),
            revenue: "30".into(),
            max_count: 2,
            percent: "50".into(),
            ..base()
        };
        assert_eq!(
            d.syntax(&data).unwrap(),
            "RESPONSE RATE /VARIABLES NUMERATOR=q1_satisfaction DENOMINATOR=age GROUP=region\n  \
             /MINRATE FORMULA=0.6, 30\n  /MAXCOUNT PERCENT=50."
        );
        let d = ResponseRate {
            min_rate: 1,
            rate: "5".into(),
            max_count: 1,
            count: "600".into(),
            ..base()
        };
        assert!(
            d.syntax(&data)
                .unwrap()
                .ends_with("/MINRATE RATE=5\n  /MAXCOUNT COUNT=600.")
        );
    }

    #[test]
    fn limits_are_checked() {
        let data = open("survey_basic.sav").dataset().unwrap().clone();
        let bad = |d: ResponseRate| d.syntax(&data).unwrap_err();
        assert!(
            bad(ResponseRate {
                group: String::new(),
                ..base()
            })
            .contains("grouping")
        );
        assert!(
            bad(ResponseRate {
                numerator: "plan".into(),
                ..base()
            })
            .contains("string")
        );
        assert!(
            bad(ResponseRate {
                min_rate: 1,
                rate: "100".into(),
                ..base()
            })
            .contains("below 100")
        );
        assert!(
            bad(ResponseRate {
                max_count: 1,
                count: "2.5".into(),
                ..base()
            })
            .contains("whole")
        );
        assert!(
            bad(ResponseRate {
                max_count: 2,
                percent: "0".into(),
                ..base()
            })
            .contains("percentage")
        );
    }
}
