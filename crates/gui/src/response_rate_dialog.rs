//! The Response Rate dialog (RESPONSE RATE): response rates of a numerator over a denominator
//! in each group, with cumulative rates and limits. Limits left off add nothing to the syntax.

use crate::i18n::t;
use crate::stats_dialogs::{numeric, parse};
use crate::syntax_text::{names, number};
use statscraft_data::Dataset;

/// What /MINRATE sets, the command's own choice first.
pub const MIN_RATES: [(&str, &str); 3] = [
    ("NONE", "stats-resprate-minrate-none"),
    ("RATE", "stats-resprate-minrate-rate"),
    ("FORMULA", "stats-resprate-minrate-formula"),
];

/// What /MAXCOUNT sets, the command's own choice first.
pub const MAX_COUNTS: [(&str, &str); 3] = [
    ("ALL", "stats-resprate-maxcount-all"),
    ("COUNT", "stats-resprate-maxcount-count"),
    ("PERCENT", "stats-resprate-maxcount-percent"),
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
        for (value, message) in [
            (&self.numerator, t("stats-resprate-set-numerator")),
            (&self.denominator, t("stats-resprate-set-denominator")),
            (&self.group, t("stats-resprate-set-group")),
        ] {
            if value.trim().is_empty() {
                return Err(message.into());
            }
        }
        let list = [
            self.numerator.clone(),
            self.denominator.clone(),
            self.group.clone(),
        ];
        names(&list, t("stats-resprate-variables"))?;
        let mut text = format!(
            "RESPONSE RATE /VARIABLES NUMERATOR={} DENOMINATOR={} GROUP={}",
            self.numerator.trim(),
            self.denominator.trim(),
            self.group.trim()
        );
        match self.min_rate {
            1 => {
                let rate = parse(&self.rate, t("stats-resprate-what-lowest-rate"))?;
                if rate <= 0.0 || rate >= 100.0 {
                    return Err(t("stats-resprate-rate-range").into());
                }
                text.push_str(&format!("\n  /MINRATE RATE={}", number(rate)));
            }
            2 => {
                let cost = parse(&self.cost, t("stats-resprate-what-cost"))?;
                let revenue = parse(&self.revenue, t("stats-resprate-what-revenue"))?;
                if cost <= 0.0 || revenue <= 0.0 || cost >= revenue {
                    return Err(t("stats-resprate-cost-revenue").into());
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
                let n = parse(&self.count, t("stats-resprate-what-highest-count"))?;
                if n < 1.0 || n.fract() != 0.0 {
                    return Err(t("stats-resprate-count-whole").into());
                }
                text.push_str(&format!("\n  /MAXCOUNT COUNT={}", number(n)));
            }
            2 => {
                let p = parse(&self.percent, t("stats-resprate-what-percentage"))?;
                if p <= 0.0 || p >= 100.0 {
                    return Err(t("stats-resprate-percent-range").into());
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
