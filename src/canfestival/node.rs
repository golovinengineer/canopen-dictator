//! The object dictionary as CanFestival sees it after importing an EDS
//! (`eds_utils.py: GenerateNode`).
//!
//! Differences from the project model, all inherited from CanFestival:
//! - entries known to its DS-301 table take name, sub-entry names, types and
//!   access rights from that table, not from the project;
//! - other ARRAY/RECORD objects become "one variable per subindex" entries
//!   whose sub 0 is a read-only `Number of Entries` UNS8;
//! - holes in the subindex numbering are filled with `Compatibility Entry`;
//! - sub 0 holds the highest subindex, whatever default the project gives;
//! - `$NODEID+x` defaults are evaluated with the configured node id.
//!
//! Deliberate deviations from the Python tool:
//! - decimal defaults with a leading zero are read as decimal, not octal.

use std::collections::BTreeMap;

use anyhow::{Result, anyhow, bail};

use super::ds301::{self, Default as TableDefault};
use crate::model::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i128),
    Real(f64),
    Str(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CfSub {
    pub name: String,
    pub data_type: DataType,
    /// `RO`, `WO`, `RW` or `CONST`.
    pub access: &'static str,
    pub value: Value,
    pub save: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CfEntry {
    pub name: String,
    /// `ds301::OD_*` flags.
    pub structure: u8,
    /// Value is a list (ARRAY/RECORD) rather than a single VAR.
    pub is_list: bool,
    pub callback: bool,
    /// Indexed by subindex, no holes.
    pub subs: Vec<CfSub>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CfNode {
    pub name: String,
    pub id: u8,
    pub node_type: NodeType,
    pub default_string_size: usize,
    pub entries: BTreeMap<u16, CfEntry>,
}

fn access_upper(a: &str) -> &'static str {
    match a {
        "ro" => "RO",
        "wo" => "WO",
        "const" => "CONST",
        _ => "RW",
    }
}

/// `ACCESS_TRANSLATE` in eds_utils.py.
fn access_from_model(a: Access) -> &'static str {
    match a {
        Access::Ro => "RO",
        Access::Wo => "WO",
        Access::Rw | Access::Rwr | Access::Rww => "RW",
        Access::Const => "CONST",
    }
}

fn type_default(t: DataType) -> Value {
    match t {
        DataType::VisibleString
        | DataType::OctetString
        | DataType::UnicodeString
        | DataType::Domain => Value::Str(String::new()),
        DataType::Real32 | DataType::Real64 => Value::Real(0.0),
        _ => Value::Int(0),
    }
}

fn table_default(d: TableDefault, t: DataType, node_id: u8, pdo_number: u32) -> Value {
    let id = node_id as i128;
    match d {
        TableDefault::Type => type_default(t),
        TableDefault::NodeId(offset) => Value::Int(id + offset as i128),
        TableDefault::RpdoCobId if pdo_number < 4 => {
            Value::Int(id + ((pdo_number as i128 + 2) << 8))
        }
        TableDefault::TpdoCobId if pdo_number < 4 => {
            Value::Int(id + ((pdo_number as i128 + 1) << 8) + 0x80)
        }
        TableDefault::RpdoCobId | TableDefault::TpdoCobId => Value::Int(0x8000_0000),
    }
}

/// Interpret a default written in the project for the given type.
/// `None` when the project gives no default.
fn parse_default(text: &str, t: DataType, node_id: u8, ctx: &str) -> Result<Option<Value>> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    Ok(Some(match t {
        DataType::VisibleString
        | DataType::OctetString
        | DataType::UnicodeString
        | DataType::Domain => Value::Str(text.to_string()),
        DataType::Real32 | DataType::Real64 => Value::Real(
            text.parse::<f64>()
                .map_err(|_| anyhow!("{ctx}: default {text:?} is not a number"))?,
        ),
        _ => {
            let upper = text.to_uppercase();
            if let Some(rest) = upper.strip_prefix("$NODEID") {
                let rest = rest.trim();
                let offset = match rest.strip_prefix('+') {
                    Some(o) => parse_int(&o.trim().to_lowercase())
                        .ok_or_else(|| anyhow!("{ctx}: bad $NODEID formula {text:?}"))?,
                    None if rest.is_empty() => 0,
                    None => bail!("{ctx}: bad $NODEID formula {text:?}"),
                };
                Value::Int(node_id as i128 + offset)
            } else if upper == "TRUE" {
                Value::Int(1)
            } else if upper == "FALSE" {
                Value::Int(0)
            } else {
                Value::Int(
                    parse_int(text)
                        .ok_or_else(|| anyhow!("{ctx}: default {text:?} is not a number"))?,
                )
            }
        }
    }))
}

fn table_type(ty: u16, ctx: &str) -> Result<DataType> {
    DataType::from_cia_index(ty).ok_or_else(|| {
        anyhow!("{ctx}: CanFestival data type 0x{ty:04X} is not supported by this generator")
    })
}

impl CfNode {
    pub fn from_model(od: &ObjectDictionary) -> Result<CfNode> {
        let cf = &od.canfestival;
        let mut entries = BTreeMap::new();

        for (index, obj) in od.enabled() {
            let ctx = format!("object 0x{index:04X}");
            let is_list = obj.kind != ObjectKind::Var;
            let table = ds301::find(index);
            let pdo_number = table.map(|t| (index - t.index) as u32).unwrap_or(0);

            if let Some(t) = table {
                let table_is_list = t.structure & ds301::OD_MULTIPLE_SUBINDEXES != 0;
                if table_is_list != is_list {
                    bail!(
                        "{ctx}: is a {} in the project but CanFestival defines \"{}\" as {}",
                        obj.kind.short(),
                        ds301::entry_name(t, index),
                        if table_is_list { "an array/record" } else { "a var" }
                    );
                }
            }

            // Sub-entry description: (name, type, access, table default).
            let describe = |sub: u8| -> Result<(String, DataType, &'static str, TableDefault)> {
                let sctx = format!("{ctx} sub 0x{sub:02X}");
                if let Some(t) = table
                    && let Some(si) = ds301::sub_info(t, sub) {
                        return Ok((
                            ds301::format_name(si.name, pdo_number + 1, sub as i64),
                            table_type(si.ty, &sctx)?,
                            access_upper(si.access),
                            si.default,
                        ));
                    }
                    // Beyond what the table describes: CanFestival itself
                    // fails here; fall back to the project's description.
                Ok(match obj.sub(sub) {
                    Some(s) if !is_list => (
                        obj.name.clone(),
                        s.data_type,
                        access_from_model(s.access),
                        TableDefault::Type,
                    ),
                    _ if sub == 0 => (
                        "Number of Entries".to_string(),
                        DataType::U8,
                        "RO",
                        TableDefault::Type,
                    ),
                    Some(s) => (
                        s.name.clone(),
                        s.data_type,
                        access_from_model(s.access),
                        TableDefault::Type,
                    ),
                    None => (
                        "Compatibility Entry".to_string(),
                        DataType::U8,
                        "RW",
                        TableDefault::Type,
                    ),
                })
            };

            let max_sub = if is_list { obj.max_sub() } else { 0 };
            let mut subs = Vec::with_capacity(max_sub as usize + 1);
            for sub in 0..=max_sub {
                let sctx = format!("{ctx} sub 0x{sub:02X}");
                let (name, data_type, access, default) = describe(sub)?;
                let project_sub = obj.sub(sub);
                let value = if is_list && sub == 0 {
                    Value::Int(max_sub as i128)
                } else {
                    let given = match project_sub {
                        Some(s) => parse_default(&s.default, data_type, cf.node_id, &sctx)?,
                        None => None,
                    };
                    given.unwrap_or_else(|| table_default(default, data_type, cf.node_id, pdo_number))
                };
                subs.push(CfSub {
                    name,
                    data_type,
                    access,
                    value,
                    save: project_sub.is_some_and(|s| s.save),
                });
            }

            let (name, structure, table_callback) = match table {
                Some(t) => (ds301::entry_name(t, index), t.structure, t.callback),
                None => (
                    obj.name.clone(),
                    if is_list { ds301::ARRAY } else { ds301::VAR },
                    false,
                ),
            };
            entries.insert(
                index,
                CfEntry {
                    name,
                    structure,
                    is_list,
                    callback: table_callback || obj.callback,
                    subs,
                },
            );
        }

        Ok(CfNode {
            name: cf.node_name.clone(),
            id: cf.node_id,
            node_type: cf.node_type,
            default_string_size: cf.default_string_size as usize,
            entries,
        })
    }
}
