//! Transformation Plan 0.1 and 0.2: the typed model, structural and catalog-free validation,
//! and the plan hash.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

pub const PLAN_V01: &str = "openstatspec-transformation-plan-v0.1";
pub const PLAN_V02: &str = "openstatspec-transformation-plan-v0.2";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub contract: String,
    pub input_alias: String,
    pub operations: Vec<Op>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    Recode {
        source: String,
        target: String,
        target_mode: TargetMode,
        rules: Vec<Rule>,
        unmatched: Outcome,
    },
    Assign {
        target: String,
        target_mode: TargetMode,
        value: Operand,
    },
    ConditionalAssign {
        condition: Predicate,
        target: String,
        value: Operand,
    },
    SetVariableLabel {
        variable: String,
        label: String,
    },
    ReplaceValueLabels {
        variable: String,
        labels: Vec<ValueLabel>,
    },
    SetFormat {
        variable: String,
        family: String,
        width: u64,
        decimals: u64,
    },
    SetMeasurementLevel {
        variable: String,
        level: Level,
    },
    // A struct variant, so that deny_unknown_fields also applies to it.
    Execute {},
}

impl Op {
    /// True for the operations that exist only in Plan 0.2.
    pub fn needs_v02(&self) -> bool {
        !matches!(
            self,
            Op::Recode { .. } | Op::SetVariableLabel { .. } | Op::ReplaceValueLabels { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetMode {
    Create,
    Replace,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Nominal,
    Ordinal,
    Scale,
}

/// IEEE 754 binary64 bits, written as 16 lowercase hex digits. Deserializing rejects negative
/// zero, infinities and NaN, as the schemas do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bits(pub u64);

impl Bits {
    pub fn parse(s: &str) -> Option<Bits> {
        let ok = s.len() == 16
            && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            && s != "8000000000000000"
            && !s.starts_with("7ff")
            && !s.starts_with("fff");
        ok.then(|| Bits(u64::from_str_radix(s, 16).expect("checked hex")))
    }

    pub fn value(self) -> f64 {
        f64::from_bits(self.0)
    }
}

impl Serialize for Bits {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{:016x}", self.0))
    }
}

impl<'de> Deserialize<'de> for Bits {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Bits::parse(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid binary64 bits {s}")))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Typed {
    Binary64 { bits: Bits },
    String { value: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueLabel {
    pub value: Typed,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    #[serde(rename = "match")]
    pub matches: Match,
    pub result: Outcome,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Match {
    Values { values: Vec<Typed> },
    Range { lower: Typed, upper: Typed },
    SystemMissing {},
}

/// The result of a recode rule, or of its unmatched case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Literal { value: Typed },
    SystemMissing {},
    Copy {},
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operand {
    Variable { variable: String },
    Literal { value: Typed },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "expression", rename_all = "snake_case", deny_unknown_fields)]
pub enum Predicate {
    Comparison {
        left: Operand,
        operator: Comparator,
        right: Operand,
    },
    Boolean {
        operator: Connective,
        operands: Vec<Predicate>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Comparator {
    #[serde(rename = "=")]
    Eq,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Ge,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Connective {
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanError {
    pub code: &'static str,
    pub message: String,
}

fn fail<T>(code: &'static str, message: impl Into<String>) -> Result<T, PlanError> {
    Err(PlanError {
        code,
        message: message.into(),
    })
}

impl Plan {
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("plain data")
    }

    /// The canonical JSON text of the plan: the bytes the plan hash covers.
    pub fn canonical(&self) -> String {
        crate::canonical_json(&self.to_json()).expect("plans hold only small integers")
    }

    pub fn hash(&self) -> String {
        crate::sha256_hex(self.canonical().as_bytes())
    }
}

/// Validates a plan the way it can be validated without a catalog: first the schema named by
/// its contract (`plan_schema_invalid`), then the rules that need no bound schema. Binding
/// checks (unknown variables, target state, storage kinds) belong to the executor.
pub fn validate_plan(value: &Value) -> Result<Plan, PlanError> {
    let invalid = |m: String| PlanError {
        code: "plan_schema_invalid",
        message: m,
    };
    let plan: Plan = serde_json::from_value(value.clone()).map_err(|e| invalid(e.to_string()))?;
    check_structure(&plan).map_err(invalid)?;
    for op in &plan.operations {
        check_semantics(op)?;
    }
    Ok(plan)
}

/// The schema rules serde does not express: constants, lengths and the operation set.
fn check_structure(plan: &Plan) -> Result<(), String> {
    let v02 = match plan.contract.as_str() {
        PLAN_V01 => false,
        PLAN_V02 => true,
        other => return Err(format!("unknown plan contract {other}")),
    };
    let nonempty = |s: &str, what: &str| {
        if s.is_empty() {
            Err(format!("{what} must not be empty"))
        } else {
            Ok(())
        }
    };
    nonempty(&plan.input_alias, "input_alias")?;
    if plan.operations.is_empty() {
        return Err("a plan needs at least one operation".into());
    }
    for op in &plan.operations {
        if op.needs_v02() && !v02 {
            return Err("this operation needs plan contract v0.2".into());
        }
        match op {
            Op::Recode {
                source,
                target,
                rules,
                ..
            } => {
                nonempty(source, "source")?;
                nonempty(target, "target")?;
                if rules.is_empty() {
                    return Err("recode needs at least one rule".into());
                }
                for rule in rules {
                    match &rule.matches {
                        Match::Values { values } if values.is_empty() => {
                            return Err("a values match needs at least one value".into());
                        }
                        Match::Range { lower, upper }
                            if !matches!(lower, Typed::Binary64 { .. })
                                || !matches!(upper, Typed::Binary64 { .. }) =>
                        {
                            return Err("range bounds must be binary64".into());
                        }
                        _ => {}
                    }
                }
            }
            Op::Assign { target, value, .. } => {
                nonempty(target, "target")?;
                operand_structure(value)?;
            }
            Op::ConditionalAssign {
                condition,
                target,
                value,
            } => {
                nonempty(target, "target")?;
                operand_structure(value)?;
                predicate_structure(condition)?;
            }
            Op::SetVariableLabel { variable, .. } | Op::SetMeasurementLevel { variable, .. } => {
                nonempty(variable, "variable")?
            }
            Op::ReplaceValueLabels { variable, labels } => {
                nonempty(variable, "variable")?;
                if labels.is_empty() {
                    return Err("replace_value_labels needs at least one label".into());
                }
            }
            Op::SetFormat {
                variable,
                family,
                width,
                decimals,
            } => {
                nonempty(variable, "variable")?;
                if family != "F" || !(1..=40).contains(width) || *decimals > 16 {
                    return Err("set_format needs family F, width 1-40 and decimals 0-16".into());
                }
            }
            Op::Execute {} => {}
        }
    }
    Ok(())
}

fn operand_structure(operand: &Operand) -> Result<(), String> {
    match operand {
        Operand::Variable { variable } if variable.is_empty() => {
            Err("variable must not be empty".into())
        }
        _ => Ok(()),
    }
}

fn predicate_structure(p: &Predicate) -> Result<(), String> {
    match p {
        Predicate::Comparison { left, right, .. } => {
            operand_structure(left)?;
            operand_structure(right)
        }
        Predicate::Boolean { operands, .. } if operands.len() < 2 => {
            Err("a boolean node needs at least two operands".into())
        }
        Predicate::Boolean { operands, .. } => operands.iter().try_for_each(predicate_structure),
    }
}

fn check_semantics(op: &Op) -> Result<(), PlanError> {
    let reserved = |target: &str| {
        if target.starts_with("__") {
            fail(
                "reserved_target_name",
                format!("{target}: names beginning with __ are reserved"),
            )
        } else {
            Ok(())
        }
    };
    match op {
        Op::Recode {
            target,
            rules,
            unmatched,
            ..
        } => {
            reserved(target)?;
            for rule in rules {
                if let Match::Range {
                    lower: Typed::Binary64 { bits: lo },
                    upper: Typed::Binary64 { bits: hi },
                } = &rule.matches
                    && lo.value() > hi.value()
                {
                    return fail(
                        "invalid_numeric_range",
                        "range lower bound is above its upper bound",
                    );
                }
            }
            let kinds: Vec<bool> = rules
                .iter()
                .map(|r| &r.result)
                .chain([unmatched])
                .filter_map(|o| match o {
                    Outcome::Literal { value } => Some(matches!(value, Typed::String { .. })),
                    _ => None,
                })
                .collect();
            if kinds.windows(2).any(|w| w[0] != w[1]) {
                return fail(
                    "mixed_result_types",
                    "recode results mix numbers and strings",
                );
            }
        }
        Op::Assign { target, value, .. } => {
            reserved(target)?;
            numeric_operand(value)?;
        }
        Op::ConditionalAssign {
            condition,
            target,
            value,
        } => {
            reserved(target)?;
            check_predicate(condition, None)?;
            numeric_operand(value)?;
        }
        Op::ReplaceValueLabels { labels, .. } => {
            for (i, label) in labels.iter().enumerate() {
                if labels[..i].iter().any(|l| l.value == label.value) {
                    return fail("duplicate_value_label", "a value has two labels");
                }
            }
        }
        Op::SetFormat {
            width, decimals, ..
        } => {
            if *decimals > 0 && *width < decimals + 2 {
                return fail(
                    "invalid_format",
                    format!(
                        "F{width}.{decimals}: the width leaves no room for sign, point and decimals"
                    ),
                );
            }
        }
        Op::SetVariableLabel { .. } | Op::SetMeasurementLevel { .. } | Op::Execute {} => {}
    }
    Ok(())
}

fn numeric_operand(operand: &Operand) -> Result<(), PlanError> {
    match operand {
        Operand::Literal {
            value: Typed::String { .. },
        } => fail(
            "expression_type_unsupported",
            "string operands are not supported",
        ),
        _ => Ok(()),
    }
}

fn check_predicate(p: &Predicate, parent: Option<Connective>) -> Result<(), PlanError> {
    match p {
        Predicate::Comparison { left, right, .. } => {
            numeric_operand(left)?;
            numeric_operand(right)
        }
        Predicate::Boolean { operator, .. } if Some(*operator) == parent => fail(
            "noncanonical_boolean_shape",
            "a boolean node nests another with the same operator",
        ),
        Predicate::Boolean { operator, operands } => operands
            .iter()
            .try_for_each(|o| check_predicate(o, Some(*operator))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn code(v: Value) -> &'static str {
        validate_plan(&v).unwrap_err().code
    }

    #[test]
    fn schema_rejects_unknown_fields_bad_bits_and_wrong_contract() {
        let execute =
            |op: Value| json!({"contract": PLAN_V02, "input_alias": "d", "operations": [op]});
        assert!(validate_plan(&execute(json!({"op": "execute"}))).is_ok());
        assert_eq!(
            code(execute(json!({"op": "execute", "x": 1}))),
            "plan_schema_invalid"
        );
        let assign = |bits: &str| {
            execute(
                json!({"op": "assign", "target": "t", "target_mode": "replace",
                "value": {"kind": "literal", "value": {"type": "binary64", "bits": bits}}}),
            )
        };
        assert!(validate_plan(&assign("3ff0000000000000")).is_ok());
        for bad in [
            "8000000000000000",
            "7ff0000000000000",
            "fff8000000000000",
            "3FF0000000000000",
            "3ff",
        ] {
            assert_eq!(code(assign(bad)), "plan_schema_invalid", "{bad}");
        }
        let mut v01 = execute(json!({"op": "execute"}));
        v01["contract"] = json!(PLAN_V01);
        assert_eq!(code(v01), "plan_schema_invalid");
    }

    #[test]
    fn mixed_literal_results_are_rejected() {
        let v = json!({"contract": PLAN_V01, "input_alias": "d", "operations": [{
            "op": "recode", "source": "s", "target": "t", "target_mode": "create",
            "rules": [{"match": {"kind": "system_missing"},
                       "result": {"kind": "literal", "value": {"type": "string", "value": "x"}}}],
            "unmatched": {"kind": "literal", "value": {"type": "binary64", "bits": "0000000000000000"}}}]});
        assert_eq!(code(v), "mixed_result_types");
    }
}
