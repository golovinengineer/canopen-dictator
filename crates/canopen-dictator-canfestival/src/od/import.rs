// SPDX-License-Identifier: LGPL-2.1-or-later
// Part of the Rust port of CanFestival objdictgen; see lib.rs for copyright.

//! `.od` -> project (`node.Node` -> [`ObjectDictionary`]).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};

use super::pickle::{Py, parse_pickle};
use crate::ds301::{self, OD_IDENTICAL_INDEXES, OD_IDENTICAL_SUBINDEXES, OD_MULTIPLE_SUBINDEXES};
use canopen_dictator_core::model::*;

/// The imported project plus what could not be carried over.
#[derive(Debug)]
pub struct Import {
    pub od: ObjectDictionary,
    pub warnings: Vec<String>,
}

pub fn load(path: &Path) -> Result<Import> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse(&text).with_context(|| format!("parsing {}", path.display()))
}

// --------------------------------------------------------- mapping tables

/// One sub-entry description of a mapping (`values` item in node.py).
#[derive(Debug, Clone)]
struct SubDef {
    name: String,
    ty: i128,
    access: String,
    pdo: bool,
    nbmax: Option<i128>,
}

/// One entry of a user/profile mapping dictionary.
#[derive(Debug, Clone)]
struct EntryDef {
    name: String,
    structure: u8,
    incr: i128,
    nbmax: i128,
    values: Vec<SubDef>,
}

type Mapping = BTreeMap<u16, EntryDef>;

fn parse_mapping(py: Option<&Py>, what: &str) -> Result<Mapping> {
    let mut out = Mapping::new();
    let Some(Py::Dict(items)) = py else {
        return Ok(out);
    };
    for (k, v) in items {
        let index = k
            .as_int()
            .ok_or_else(|| anyhow!("{what}: non-numeric index {k:?}"))?;
        // Below 0x1000 are data type definitions, not entries.
        if !(0x1000..=0xFFFF).contains(&index) {
            continue;
        }
        let ctx = format!("{what} 0x{index:04X}");
        let field = |key: &str| v.get_str(key);
        let values = match field("values") {
            Some(Py::List(l)) => l
                .iter()
                .map(|s| {
                    Ok(SubDef {
                        name: s
                            .get_str("name")
                            .and_then(Py::as_str)
                            .unwrap_or("")
                            .to_string(),
                        ty: s
                            .get_str("type")
                            .and_then(Py::as_int)
                            .ok_or_else(|| anyhow!("{ctx}: sub-entry without a type"))?,
                        access: s
                            .get_str("access")
                            .and_then(Py::as_str)
                            .unwrap_or("rw")
                            .to_string(),
                        pdo: s.get_str("pdo").is_some_and(Py::truthy),
                        nbmax: s.get_str("nbmax").and_then(Py::as_int),
                    })
                })
                .collect::<Result<_>>()?,
            _ => Vec::new(),
        };
        out.insert(
            index as u16,
            EntryDef {
                name: field("name").and_then(Py::as_str).unwrap_or("").to_string(),
                structure: field("struct").and_then(Py::as_int).unwrap_or(0) as u8,
                incr: field("incr").and_then(Py::as_int).unwrap_or(1).max(1),
                nbmax: field("nbmax").and_then(Py::as_int).unwrap_or(0),
                values,
            },
        );
    }
    Ok(out)
}

/// `FindIndex`: the base index of the mapping entry describing `index`.
fn find_index(index: u16, mapping: &Mapping) -> Option<u16> {
    if mapping.contains_key(&index) {
        return Some(index);
    }
    let i = index as i128;
    mapping
        .iter()
        .filter(|(_, e)| e.structure & OD_IDENTICAL_INDEXES != 0)
        .find(|(base, e)| {
            let b = **base as i128;
            b < i && i < b + e.incr * e.nbmax && (i - b) % e.incr == 0
        })
        .map(|(base, _)| *base)
}

/// `FindSubentryInfos` on one mapping entry, name formatted.
fn find_sub(e: &EntryDef, base: u16, index: u16, sub: u8) -> Option<SubDef> {
    let sub_i = sub as i128;
    let incr = if e.structure & OD_IDENTICAL_INDEXES != 0 {
        e.incr
    } else {
        1
    };
    let info = if e.structure & ds301::OD_SUBINDEX == 0 {
        None
    } else if e.structure & OD_IDENTICAL_SUBINDEXES != 0 {
        if sub == 0 {
            e.values.first()
        } else {
            e.values.get(1).filter(|s| sub_i <= s.nbmax.unwrap_or(0))
        }
    } else if e.structure & OD_MULTIPLE_SUBINDEXES != 0 {
        let mut idx = 0;
        let mut found = None;
        for s in &e.values {
            let n = s.nbmax.unwrap_or(1);
            if idx <= sub_i && sub_i < idx + n {
                found = Some(s);
                break;
            }
            idx += n;
        }
        found
    } else if sub == 0 {
        e.values.first()
    } else {
        None
    }?;
    let mut info = info.clone();
    let idx = ((index - base) as i128 / incr + 1) as u32;
    info.name = ds301::format_name(&info.name, idx, sub as i64);
    Some(info)
}

/// Names of CanFestival types the model does not have (yet).
fn cf_type_name(ty: i128) -> Option<&'static str> {
    Some(match ty {
        0x0C => "TIME_OF_DAY",
        0x0D => "TIME_DIFFERENCE",
        0x10 => "INTEGER24",
        0x12 => "INTEGER40",
        0x13 => "INTEGER48",
        0x14 => "INTEGER56",
        0x16 => "UNSIGNED24",
        0x18 => "UNSIGNED40",
        0x19 => "UNSIGNED48",
        0x1A => "UNSIGNED56",
        _ => return None,
    })
}

fn data_type(ty: i128, ctx: &str) -> Result<DataType> {
    if let Some(t) = u16::try_from(ty).ok().and_then(DataType::from_cia_index) {
        return Ok(t);
    }
    match cf_type_name(ty) {
        Some(name) => bail!("{ctx}: data type {name} is not supported yet"),
        None if (0xA0..0x1000).contains(&ty) => {
            bail!("{ctx}: user-defined value range type 0x{ty:04X} is not supported yet")
        }
        None => bail!("{ctx}: unknown data type 0x{ty:04X}"),
    }
}

fn access(a: &str, ctx: &str) -> Result<Access> {
    Access::from_short(&a.to_ascii_lowercase())
        .ok_or_else(|| anyhow!("{ctx}: unknown access {a:?}"))
}

// ---------------------------------------------------------------- values

/// Text form of a stored value, in the style of the project file:
/// unsigned integers as zero-padded hex, everything else as written.
fn format_value(v: &Py, t: DataType, ctx: &str) -> Result<String> {
    Ok(match v {
        Py::None => String::new(),
        Py::Bool(b) => (if *b { "1" } else { "0" }).to_string(),
        Py::Int(i) => {
            let width = match t {
                DataType::U8 => 2,
                DataType::U16 => 4,
                DataType::U32 => 8,
                DataType::U64 => 16,
                _ => 0,
            };
            if width > 0 && *i >= 0 {
                format!("0x{i:0width$X}")
            } else {
                i.to_string()
            }
        }
        Py::Float(f) => f.clone(),
        Py::Str(s) if s.to_ascii_uppercase().contains("$NODEID") => {
            // Stored as Python source of a string: `"$NODEID+0x180"`.
            let t = s.trim();
            let unquoted = ['"', '\'']
                .iter()
                .find_map(|q| t.strip_prefix(*q).and_then(|r| r.strip_suffix(*q)));
            unquoted.unwrap_or(t).to_string()
        }
        Py::Str(s) => s.clone(),
        other => bail!("{ctx}: unexpected value {other:?}"),
    })
}

/// Evaluate the PDO COB-ID default of CanFestival's DS-301 table, stored as
/// Python source: `{True:"$NODEID+0x%X00"%(base+2),False:0x80000000}[base<4]`.
pub(crate) fn eval_base_formula(s: &str, base: u32) -> Option<String> {
    let between = |from: &str, to: &str| -> Option<&str> {
        let start = s.find(from)? + from.len();
        let len = s[start..].find(to)?;
        Some(&s[start..start + len])
    };
    let fmt = between("{True:\"", "\"%(base+")?;
    let k: u32 = between("%(base+", "),False:")?.trim().parse().ok()?;
    let otherwise = between("False:", "}[base<")?.trim();
    let limit: u32 = between("}[base<", "]")?.trim().parse().ok()?;
    Some(if base < limit {
        fmt.replacen("%X", &format!("{:X}", base + k), 1)
    } else {
        otherwise.to_string()
    })
}

// ---------------------------------------------------------------- import

/// Sub-entry as resolved for one index: name, type, access, PDO flag.
struct SubDesc {
    name: String,
    data_type: DataType,
    access: Access,
    pdo: bool,
}

pub fn parse(text: &str) -> Result<Import> {
    let node = parse_pickle(text)?;
    match &node {
        Py::Object { class, .. } if class == "Node" => {}
        _ => bail!("not a CanFestival .od file (no node.Node object)"),
    }
    from_node(&node)
}

fn from_node(node: &Py) -> Result<Import> {
    let mut warnings = Vec::new();
    let text = |name: &str| {
        node.attr(name)
            .and_then(Py::as_str)
            .unwrap_or("")
            .to_string()
    };

    let mappings = [
        parse_mapping(node.attr("Profile"), "Profile")?,
        parse_mapping(node.attr("DS302"), "DS302")?,
        parse_mapping(node.attr("UserMapping"), "UserMapping")?,
    ];
    let user_entry = |index: u16| {
        mappings
            .iter()
            .find_map(|m| find_index(index, m).map(|base| (base, &m[&base])))
    };
    let params = node
        .attr("ParamsDictionary")
        .cloned()
        .unwrap_or(Py::Dict(Vec::new()));

    let mut od = ObjectDictionary::default();
    od.device.granularity = 8;
    od.device.baud_rates = vec![1000];
    od.device.file_version = "1".into();
    od.device.created = now_timestamp();
    od.device.product_name = text("Name");
    od.device.description = text("Description");
    od.canfestival.node_name = text("Name");
    od.canfestival.node_type = match text("Type").as_str() {
        "master" => NodeType::Master,
        _ => NodeType::Slave,
    };
    if let Some(size) = node.attr("DefaultStringSize").and_then(Py::as_int) {
        od.canfestival.default_string_size = size as u32;
    }
    if let Some(id) = node.attr("ID").and_then(Py::as_int).filter(|id| *id != 0) {
        warnings.push(format!(
            "node id {id} in the file ignored (objdictedit resets it to 0 on open); set it in the generator settings if needed"
        ));
    }

    let Some(Py::Dict(dictionary)) = node.attr("Dictionary") else {
        bail!("no Dictionary in the node");
    };
    let mut entries: Vec<(u16, &Py)> = Vec::new();
    for (k, v) in dictionary {
        let index = k
            .as_int()
            .ok_or_else(|| anyhow!("Dictionary: non-numeric index {k:?}"))?;
        match u16::try_from(index) {
            Ok(i) if i >= 0x1000 => entries.push((i, v)),
            _ => warnings.push(format!(
                "user-defined type 0x{index:04X} skipped (value range types are not supported yet)"
            )),
        }
    }
    entries.sort_by_key(|(i, _)| *i);

    for (index, value) in entries {
        let ctx = format!("object 0x{index:04X}");
        let user = user_entry(index);
        let table = ds301::find(index);
        let (name, structure) = match (user, table) {
            (Some((base, e)), _) => {
                let name = if e.structure & OD_IDENTICAL_INDEXES != 0 {
                    ds301::format_name(&e.name, ((index - base) as i128 / e.incr + 1) as u32, 0)
                } else {
                    e.name.clone()
                };
                (name, e.structure)
            }
            (None, Some(t)) => (ds301::entry_name(t, index), t.structure),
            (None, None) => bail!("{ctx}: no definition in the file or in the DS-301 table"),
        };

        let describe = |sub: u8| -> Result<SubDesc> {
            let sctx = format!("{ctx} sub 0x{sub:02X}");
            if let Some((base, e)) = user
                && let Some(s) = find_sub(e, base, index, sub)
            {
                return Ok(SubDesc {
                    name: s.name,
                    data_type: data_type(s.ty, &sctx)?,
                    access: access(&s.access, &sctx)?,
                    pdo: s.pdo,
                });
            }
            if let Some(t) = table
                && let Some(s) = ds301::sub_info(t, sub)
            {
                let pdo_number = (index - t.index) as u32;
                return Ok(SubDesc {
                    name: ds301::format_name(s.name, pdo_number + 1, sub as i64),
                    data_type: data_type(s.ty as i128, &sctx)?,
                    access: access(s.access, &sctx)?,
                    pdo: s.pdo,
                });
            }
            bail!("{sctx}: no definition in the file or in the DS-301 table")
        };

        let index_params = params.get(&Py::Int(index as i128));
        let flag =
            |p: Option<&Py>, key: &str| p.and_then(|p| p.get_str(key)).is_some_and(Py::truthy);
        let comment = |p: Option<&Py>| {
            p.and_then(|p| p.get_str("comment"))
                .and_then(Py::as_str)
                .unwrap_or("")
                .to_string()
        };
        let mut check_buffer_size = |p: Option<&Py>, what: &str| {
            if let Some(b) = p.and_then(|p| p.get_str("buffer_size"))
                && b.truthy()
            {
                warnings.push(format!(
                    "{what}: buffer_size {b:?} ignored (per-entry string sizes are not supported yet)"
                ));
            }
        };

        let make_sub = |sub: u8, v: &Py| -> Result<SubObject> {
            let d = describe(sub)?;
            let sctx = format!("{ctx} sub 0x{sub:02X}");
            let mut s = SubObject::new(sub, &d.name, d.data_type, d.access);
            s.pdo = if d.pdo {
                PdoMapping::Optional
            } else {
                PdoMapping::No
            };
            s.default = match (v, table) {
                (Py::Str(f), Some(t)) if f.contains("[base<") => {
                    eval_base_formula(f, (index - t.index) as u32)
                        .ok_or_else(|| anyhow!("{sctx}: cannot evaluate default {f:?}"))?
                }
                _ => format_value(v, d.data_type, &sctx)?,
            };
            Ok(s)
        };

        let is_list = structure & OD_MULTIPLE_SUBINDEXES != 0;
        let mut obj = match value {
            Py::List(values) if is_list => {
                let kind = if structure & OD_IDENTICAL_SUBINDEXES != 0 {
                    ObjectKind::Array
                } else {
                    ObjectKind::Record
                };
                let mut obj = Object::new(&name, kind);
                obj.subs.clear();
                let count = u8::try_from(values.len())
                    .map_err(|_| anyhow!("{ctx}: {} subindexes", values.len()))?;
                obj.subs.push(make_sub(0, &Py::Int(count as i128))?);
                for (i, v) in values.iter().enumerate() {
                    obj.subs.push(make_sub(i as u8 + 1, v)?);
                }
                for s in obj.subs.iter_mut() {
                    let p = index_params.and_then(|p| p.get(&Py::Int(s.sub as i128)));
                    s.save = flag(p, "save");
                    s.description = comment(p);
                    check_buffer_size(p, &format!("{ctx} sub 0x{:02X}", s.sub));
                }
                // objdictedit has no field for it; Dictator's export puts it here.
                obj.description = comment(index_params);
                obj
            }
            v if !is_list && !matches!(v, Py::List(_)) => {
                let mut obj = Object::new(&name, ObjectKind::Var);
                obj.subs = vec![make_sub(0, v)?];
                // The model keeps one name per VAR (0x1006 differs in the
                // DS-301 table: "Communication Cycle Period").
                obj.subs[0].name = name.clone();
                obj.subs[0].save = flag(index_params, "save");
                obj.description = comment(index_params);
                check_buffer_size(index_params, &ctx);
                obj
            }
            _ => bail!(
                "{ctx}: value does not match its definition ({})",
                if is_list {
                    "expected a list of subindex values"
                } else {
                    "expected a single value"
                }
            ),
        };
        obj.callback = flag(index_params, "callback");
        od.objects.insert(index, obj);
    }

    let count = |range: std::ops::RangeInclusive<u16>| {
        od.objects.range(range).count().min(u16::MAX as usize) as u16
    };
    od.device.rpdo_count = count(0x1400..=0x15FF);
    od.device.tpdo_count = count(0x1800..=0x19FF);
    Ok(Import { od, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = r#"<?xml version="1.0"?>
<!DOCTYPE PyObject SYSTEM "PyObjects.dtd">
<PyObject module="node" class="Node" id="1">"#;

    fn doc(body: &str) -> String {
        format!("{HEAD}{body}</PyObject>")
    }

    #[test]
    fn pdo_cob_id_formula() {
        let rx = "{True:\"$NODEID+0x%X00\"%(base+2),False:0x80000000}[base<4]";
        assert_eq!(eval_base_formula(rx, 0).as_deref(), Some("$NODEID+0x200"));
        assert_eq!(eval_base_formula(rx, 3).as_deref(), Some("$NODEID+0x500"));
        assert_eq!(eval_base_formula(rx, 4).as_deref(), Some("0x80000000"));
    }

    #[test]
    fn values_are_formatted_like_the_project_file() {
        let f = |v: Py, t| format_value(&v, t, "").unwrap();
        assert_eq!(f(Py::Int(8), DataType::U8), "0x08");
        assert_eq!(f(Py::Int(0x180), DataType::U32), "0x00000180");
        assert_eq!(f(Py::Int(-5), DataType::I16), "-5");
        assert_eq!(f(Py::Bool(false), DataType::Boolean), "0");
        assert_eq!(
            f(Py::Str("\"$NODEID+0x180\"".into()), DataType::U32),
            "$NODEID+0x180"
        );
        assert_eq!(
            f(Py::Str("Example".into()), DataType::VisibleString),
            "Example"
        );
    }

    #[test]
    fn user_mapping_params_and_table_entries() {
        let import = parse(&doc(r#"
<attr name="Name" type="string" value="Dev" />
<attr name="Type" type="string" value="master" />
<attr name="ID" type="numeric" value="5" />
<attr name="Dictionary" type="dict" id="3">
  <entry><key type="numeric" value="4097" /><val type="numeric" value="0" /></entry>
  <entry><key type="numeric" value="8192" /><val type="list" id="4">
    <item type="numeric" value="7" /><item type="numeric" value="-2" />
  </val></entry>
  <entry><key type="numeric" value="8448" /><val type="numeric" value="300" /></entry>
</attr>
<attr name="ParamsDictionary" type="dict" id="5">
  <entry><key type="numeric" value="8192" /><val type="dict" id="6">
    <entry><key type="string" value="callback" /><val type="True" value="" /></entry>
    <entry><key type="numeric" value="2" /><val type="dict" id="7">
      <entry><key type="string" value="save" /><val type="True" value="" /></entry>
      <entry><key type="string" value="comment" /><val type="string" value="offset" /></entry>
    </val></entry>
  </val></entry>
</attr>
<attr name="UserMapping" type="dict" id="8">
  <entry><key type="numeric" value="8192" /><val type="dict" id="9">
    <entry><key type="string" value="name" /><val type="string" value="Cal" /></entry>
    <entry><key type="string" value="struct" /><val type="numeric" value="7" /></entry>
    <entry><key type="string" value="values" /><val type="list" id="10">
      <item type="dict" id="11">
        <entry><key type="string" value="name" /><val type="string" value="Number of Entries" /></entry>
        <entry><key type="string" value="type" /><val type="numeric" value="5" /></entry>
        <entry><key type="string" value="access" /><val type="string" value="ro" /></entry>
        <entry><key type="string" value="pdo" /><val type="False" value="" /></entry>
      </item>
      <item type="dict" id="12">
        <entry><key type="string" value="name" /><val type="string" value="Gain %d[(sub)]" /></entry>
        <entry><key type="string" value="type" /><val type="numeric" value="3" /></entry>
        <entry><key type="string" value="access" /><val type="string" value="rw" /></entry>
        <entry><key type="string" value="pdo" /><val type="True" value="" /></entry>
        <entry><key type="string" value="nbmax" /><val type="numeric" value="4" /></entry>
      </item>
    </val></entry>
  </val></entry>
  <entry><key type="numeric" value="8448" /><val type="dict" id="13">
    <entry><key type="string" value="name" /><val type="string" value="Speed" /></entry>
    <entry><key type="string" value="struct" /><val type="numeric" value="1" /></entry>
    <entry><key type="string" value="values" /><val type="list" id="14">
      <item type="dict" id="15">
        <entry><key type="string" value="name" /><val type="string" value="Speed" /></entry>
        <entry><key type="string" value="type" /><val type="numeric" value="6" /></entry>
        <entry><key type="string" value="access" /><val type="string" value="rw" /></entry>
        <entry><key type="string" value="pdo" /><val type="True" value="" /></entry>
      </item>
    </val></entry>
  </val></entry>
</attr>"#))
        .unwrap();
        let od = &import.od;
        assert_eq!(od.canfestival.node_name, "Dev");
        assert_eq!(od.canfestival.node_type, NodeType::Master);
        assert_eq!(od.canfestival.node_id, 0);
        assert_eq!(import.warnings.len(), 1, "{:?}", import.warnings);

        let err = &od.objects[&0x1001];
        assert_eq!(err.name, "Error Register");
        assert_eq!(err.subs[0].data_type, DataType::U8);
        assert_eq!(err.subs[0].access, Access::Ro);
        assert_eq!(err.subs[0].pdo, PdoMapping::Optional);

        let cal = &od.objects[&0x2000];
        assert_eq!(cal.kind, ObjectKind::Array);
        assert!(cal.callback);
        let names: Vec<_> = cal.subs.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Number of Entries", "Gain 1", "Gain 2"]);
        assert_eq!(cal.subs[0].default, "0x02");
        assert_eq!(cal.subs[2].default, "-2");
        assert!(cal.subs[2].save && !cal.subs[1].save);
        assert_eq!(cal.subs[2].description, "offset");

        let speed = &od.objects[&0x2100];
        assert_eq!(speed.kind, ObjectKind::Var);
        assert_eq!(speed.subs[0].default, "0x012C");
        assert!(!speed.callback);
    }

    #[test]
    fn unsupported_types_are_reported() {
        let r = parse(&doc(r#"
<attr name="Dictionary" type="dict" id="3">
  <entry><key type="numeric" value="8192" /><val type="numeric" value="0" /></entry>
</attr>
<attr name="UserMapping" type="dict" id="8">
  <entry><key type="numeric" value="8192" /><val type="dict" id="9">
    <entry><key type="string" value="name" /><val type="string" value="X" /></entry>
    <entry><key type="string" value="struct" /><val type="numeric" value="1" /></entry>
    <entry><key type="string" value="values" /><val type="list" id="10">
      <item type="dict" id="11">
        <entry><key type="string" value="name" /><val type="string" value="X" /></entry>
        <entry><key type="string" value="type" /><val type="numeric" value="22" /></entry>
        <entry><key type="string" value="access" /><val type="string" value="rw" /></entry>
      </item>
    </val></entry>
  </val></entry>
</attr>"#));
        let msg = format!("{:#}", r.unwrap_err());
        assert!(msg.contains("UNSIGNED24"), "{msg}");
    }
}
