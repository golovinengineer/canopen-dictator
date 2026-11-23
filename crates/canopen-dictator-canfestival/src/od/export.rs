// SPDX-License-Identifier: LGPL-2.1-or-later
// Part of the Rust port of CanFestival objdictgen; see lib.rs for copyright.

//! Project -> `.od` ([`ObjectDictionary`] -> `node.Node`).
//!
//! Follows `eds_utils.py: GenerateNode`, i.e. what objdictedit stored after
//! importing the EDS of the project:
//! - indexes in the DS-301 table get a value only, no mapping entry;
//! - other VARs get a `struct = 1` user mapping, other ARRAY/RECORDs a
//!   `struct = 3` one (named sub-entries, CanFestival makes no difference
//!   between the two) with sub 0 `Number of Entries` and `Compatibility
//!   Entry` for missing subindexes;
//! - values are typed as `VerifyValue` does: bool for BOOLEAN, float for
//!   REAL, `"$NODEID+x"` formulas as quoted strings;
//! - missing values take the table default (`"$NODEID+0x600"`, the PDO
//!   COB-ID formula) or the default of the type.
//!
//! The project keeps more than an `.od` can hold: limits, `rwr`/`rww`, the
//! TPDO/RPDO distinction, storage group, count label, device information
//! other than name and description, and disabled objects (left out) are not
//! written.

use anyhow::{Result, anyhow, bail};

use super::pickle::{Py, write_pickle};
use crate::ds301::{self, Default as TableDefault};
use canopen_dictator_core::model::*;

/// The `.od` file for `od`.
pub fn generate(od: &ObjectDictionary) -> Result<String> {
    Ok(write_pickle(&to_node(od)?))
}

fn s(text: &str) -> Py {
    Py::Str(text.to_string())
}

fn type_default(t: DataType) -> Py {
    match t {
        DataType::Boolean => Py::Bool(false),
        DataType::Real32 | DataType::Real64 => Py::Float("0.0".into()),
        DataType::VisibleString
        | DataType::OctetString
        | DataType::UnicodeString
        | DataType::Domain => s(""),
        _ => Py::Int(0),
    }
}

/// The `default` of a DS-301 table sub-entry, as node.py writes it.
fn table_default(d: TableDefault, t: DataType) -> Py {
    match d {
        TableDefault::Type => type_default(t),
        TableDefault::NodeId(offset) => Py::Str(format!("\"$NODEID+0x{offset:X}\"")),
        TableDefault::RpdoCobId => s("{True:\"$NODEID+0x%X00\"%(base+2),False:0x80000000}[base<4]"),
        TableDefault::TpdoCobId => s("{True:\"$NODEID+0x%X80\"%(base+1),False:0x80000000}[base<4]"),
    }
}

/// The value of a project default, typed like `eds_utils.py: VerifyValue`.
/// `None` when the project gives none.
fn value(text: &str, t: DataType, ctx: &str) -> Result<Option<Py>> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let trimmed = text.trim();
    let upper = trimmed.to_ascii_uppercase();
    let int = || -> Result<i128> {
        match upper.as_str() {
            "TRUE" => Ok(1),
            "FALSE" => Ok(0),
            _ => parse_int(trimmed).ok_or_else(|| anyhow!("{ctx}: default {text:?} is not a number")),
        }
    };
    Ok(Some(match t {
        DataType::VisibleString
        | DataType::OctetString
        | DataType::UnicodeString
        | DataType::Domain => s(text),
        DataType::Real32 | DataType::Real64 => {
            let f: f64 = trimmed
                .parse()
                .map_err(|_| anyhow!("{ctx}: default {text:?} is not a number"))?;
            Py::Float(format!("{f:?}"))
        }
        DataType::Boolean => Py::Bool(int()? != 0),
        _ if upper.starts_with("$NODEID") => Py::Str(format!("\"{trimmed}\"")),
        _ => Py::Int(int()?),
    }))
}

fn table_type(ty: u16, ctx: &str) -> Result<DataType> {
    DataType::from_cia_index(ty)
        .ok_or_else(|| anyhow!("{ctx}: CanFestival data type 0x{ty:04X} is not supported"))
}

fn sub_def(name: &str, t: DataType, access: Access, pdo: bool) -> Py {
    let access = match access {
        Access::Ro => "ro",
        Access::Wo => "wo",
        Access::Rw | Access::Rwr | Access::Rww => "rw",
        Access::Const => "const",
    };
    Py::Dict(vec![
        (s("name"), s(name)),
        (s("type"), Py::Int(t.cia_index() as i128)),
        (s("access"), s(access)),
        (s("pdo"), Py::Bool(pdo)),
    ])
}

/// The `node.Node` object for `od`.
pub fn to_node(od: &ObjectDictionary) -> Result<Py> {
    let cf = &od.canfestival;
    let mut dictionary = Vec::new();
    let mut user_mapping = Vec::new();
    let mut params = Vec::new();

    for (index, obj) in od.enabled() {
        let ctx = format!("object 0x{index:04X}");
        let is_list = obj.kind != ObjectKind::Var;
        let table = ds301::find(index);
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

        // `GetDefaultValue`: table default, else the default of the type.
        let default_for = |sub: u8, project_type: Option<DataType>| -> Result<Py> {
            let sctx = format!("{ctx} sub 0x{sub:02X}");
            if let Some(t) = table
                && let Some(si) = ds301::sub_info(t, sub)
            {
                return Ok(table_default(si.default, table_type(si.ty, &sctx)?));
            }
            Ok(type_default(project_type.unwrap_or(DataType::U8)))
        };
        let value_of = |sub: u8| -> Result<Py> {
            let sctx = format!("{ctx} sub 0x{sub:02X}");
            match obj.sub(sub) {
                Some(so) => match value(&so.default, so.data_type, &sctx)? {
                    Some(v) => Ok(v),
                    None => default_for(sub, Some(so.data_type)),
                },
                None => default_for(sub, None),
            }
        };

        let mut index_params = Vec::new();
        if obj.callback {
            index_params.push((s("callback"), Py::Bool(true)));
        }
        if !obj.description.is_empty() {
            index_params.push((s("comment"), s(&obj.description)));
        }

        if is_list {
            let max_sub = obj.max_sub();
            dictionary.push((
                Py::Int(index as i128),
                Py::List((1..=max_sub).map(value_of).collect::<Result<_>>()?),
            ));
            if table.is_none() {
                let mut values = vec![sub_def("Number of Entries", DataType::U8, Access::Ro, false)];
                for sub in 1..=max_sub {
                    values.push(match obj.sub(sub) {
                        Some(so) => sub_def(&so.name, so.data_type, so.access, so.pdo != PdoMapping::No),
                        None => sub_def("Compatibility Entry", DataType::U8, Access::Rw, false),
                    });
                }
                user_mapping.push((Py::Int(index as i128), mapping_entry(&obj.name, 3, values)));
            }
            for so in &obj.subs {
                let mut p = Vec::new();
                if so.save {
                    p.push((s("save"), Py::Bool(true)));
                }
                if !so.description.is_empty() {
                    p.push((s("comment"), s(&so.description)));
                }
                if !p.is_empty() {
                    index_params.push((Py::Int(so.sub as i128), Py::Dict(p)));
                }
            }
        } else {
            let so = obj.sub(0).ok_or_else(|| anyhow!("{ctx}: VAR without sub 0"))?;
            dictionary.push((Py::Int(index as i128), value_of(0)?));
            if table.is_none() {
                let def = sub_def(&obj.name, so.data_type, so.access, so.pdo != PdoMapping::No);
                user_mapping.push((Py::Int(index as i128), mapping_entry(&obj.name, 1, vec![def])));
            }
            if so.save {
                index_params.push((s("save"), Py::Bool(true)));
            }
        }
        if !index_params.is_empty() {
            params.push((Py::Int(index as i128), Py::Dict(index_params)));
        }
    }

    let mut attrs = vec![
        ("Profile".to_string(), Py::Dict(Vec::new())),
        ("Description".to_string(), s(&od.device.description)),
        ("Dictionary".to_string(), Py::Dict(dictionary)),
        ("SpecificMenu".to_string(), Py::List(Vec::new())),
        ("ParamsDictionary".to_string(), Py::Dict(params)),
        ("UserMapping".to_string(), Py::Dict(user_mapping)),
        ("DS302".to_string(), Py::Dict(Vec::new())),
        ("ProfileName".to_string(), s("DS-301")),
        ("Type".to_string(), s(cf.node_type.short())),
        ("ID".to_string(), Py::Int(cf.node_id as i128)),
        ("Name".to_string(), s(&cf.node_name)),
    ];
    // A class attribute in node.py; only stored when changed.
    if cf.default_string_size != CfSettings::default().default_string_size {
        attrs.push(("DefaultStringSize".to_string(), Py::Int(cf.default_string_size as i128)));
    }
    Ok(Py::Object { module: "node".into(), class: "Node".into(), attrs })
}

/// `AddMappingEntry` result: `{name, struct, need, values}`.
fn mapping_entry(name: &str, structure: u8, values: Vec<Py>) -> Py {
    Py::Dict(vec![
        (s("name"), s(name)),
        (s("struct"), Py::Int(structure as i128)),
        (s("need"), Py::Bool(false)),
        (s("values"), Py::List(values)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_typed_like_verify_value() {
        let v = |t: &str, ty| value(t, ty, "").unwrap();
        assert_eq!(v("", DataType::U32), None);
        assert_eq!(v("0x180", DataType::U32), Some(Py::Int(0x180)));
        assert_eq!(v("010", DataType::U8), Some(Py::Int(10)));
        assert_eq!(v("-3", DataType::I16), Some(Py::Int(-3)));
        assert_eq!(v("1", DataType::Boolean), Some(Py::Bool(true)));
        assert_eq!(v("2.5", DataType::Real32), Some(Py::Float("2.5".into())));
        assert_eq!(v("$NODEID+0x180", DataType::U32), Some(s("\"$NODEID+0x180\"")));
        assert_eq!(v("0x10", DataType::VisibleString), Some(s("0x10")));
        assert!(value("abc", DataType::U8, "").is_err());
    }
}
