//! Native project file (`*.objdict.toml`).
//!
//! The file is written by hand rather than through a TOML serializer so the
//! output is stable and diff-friendly: objects sorted by index, hex indexes,
//! one line per subindex, fixed key order, no timestamps, no column padding
//! (padding would re-align a whole object when one name changes).
//!
//! ```toml
//! format = 1
//!
//! [object.0x1000]            # VAR: value fields sit on the object itself
//! name = "Device type"
//! kind = "var"
//! type = "u32"
//! access = "ro"
//! default = "0x00000000"
//!
//! [object.0x2200]            # ARRAY / RECORD: one inline table per subindex
//! name = "Status General"
//! kind = "record"
//! sub = [
//!   { sub = 0x00, name = "Highest sub-index supported", type = "u8", access = "ro", default = "0x08" },
//!   { sub = 0x01, name = "Status", type = "u8", access = "rw", pdo = "tpdo", default = "0" },
//! ]
//!
//! [object.0x2200.doc]        # per-subindex descriptions, only when present
//! 0x01 = "Device status"
//! ```
//!
// TODO: preserve user comments and unknown keys on save (toml_edit).

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use toml::{Table, Value};

use crate::model::*;

pub const FORMAT_VERSION: i64 = 1;
pub const EXTENSION: &str = "objdict.toml";

pub fn load(path: &Path) -> Result<ObjectDictionary> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    from_toml(&text).with_context(|| format!("parsing {}", path.display()))
}

pub fn save(od: &ObjectDictionary, path: &Path) -> Result<()> {
    std::fs::write(path, to_toml(od)).with_context(|| format!("writing {}", path.display()))
}

// ---------------------------------------------------------------- writing

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Single-line text as a basic string, multi-line text as `"""` block.
fn quote_text(s: &str) -> String {
    if !s.contains('\n') {
        return quote(s);
    }
    let escape_quotes = s.contains("\"\"") || s.ends_with('"');
    let mut out = String::from("\"\"\"\n");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' if escape_quotes => out.push_str("\\\""),
            '\n' | '\t' => out.push(c),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push_str("\"\"\"");
    out
}

/// `key = value` pairs of one subindex, in fixed order, skipping defaults.
fn sub_fields(s: &SubObject, with_sub: bool, with_name: bool) -> Vec<(&'static str, String)> {
    let mut f = Vec::new();
    if with_sub {
        f.push(("sub", format!("0x{:02X}", s.sub)));
    }
    if with_name {
        f.push(("name", quote(&s.name)));
    }
    f.push(("type", quote(s.data_type.short())));
    f.push(("access", quote(s.access.short())));
    if s.pdo != PdoMapping::No {
        f.push(("pdo", quote(s.pdo.short())));
    }
    if !s.default.is_empty() {
        f.push(("default", quote(&s.default)));
    }
    if !s.min.is_empty() {
        f.push(("min", quote(&s.min)));
    }
    if !s.max.is_empty() {
        f.push(("max", quote(&s.max)));
    }
    if s.save {
        f.push(("save", "true".into()));
    }
    f
}

pub fn to_toml(od: &ObjectDictionary) -> String {
    let mut o = String::new();
    let d = &od.device;
    let _ = writeln!(o, "format = {FORMAT_VERSION}\n");
    let _ = writeln!(o, "[device]");
    let _ = writeln!(o, "vendor_name = {}", quote(&d.vendor_name));
    let _ = writeln!(o, "vendor_id = {}", quote(&d.vendor_id));
    let _ = writeln!(o, "product_name = {}", quote(&d.product_name));
    let _ = writeln!(o, "product_id = {}", quote(&d.product_id));
    let _ = writeln!(o, "description = {}", quote_text(&d.description));
    let _ = writeln!(o, "file_version = {}", quote(&d.file_version));
    let _ = writeln!(o, "created_by = {}", quote(&d.created_by));
    let _ = writeln!(o, "created = {}", quote(&d.created));
    let rates: Vec<String> = d.baud_rates.iter().map(|b| b.to_string()).collect();
    let _ = writeln!(o, "baud_rates = [{}]", rates.join(", "));
    let _ = writeln!(o, "granularity = {}", d.granularity);
    let _ = writeln!(o, "rpdo_count = {}", d.rpdo_count);
    let _ = writeln!(o, "tpdo_count = {}", d.tpdo_count);

    let c = &od.canfestival;
    let _ = writeln!(o, "\n[canfestival]");
    let _ = writeln!(o, "node_name = {}", quote(&c.node_name));
    let _ = writeln!(o, "node_id = {}", c.node_id);
    let _ = writeln!(o, "node_type = {}", quote(c.node_type.short()));
    let _ = writeln!(o, "default_string_size = {}", c.default_string_size);

    for (index, obj) in &od.objects {
        let _ = writeln!(o, "\n[object.0x{index:04X}]");
        let _ = writeln!(o, "name = {}", quote(&obj.name));
        let _ = writeln!(o, "kind = {}", quote(obj.kind.short()));
        if obj.disabled {
            let _ = writeln!(o, "disabled = true");
        }
        if obj.callback {
            let _ = writeln!(o, "callback = true");
        }
        if !obj.storage_group.is_empty() {
            let _ = writeln!(o, "storage_group = {}", quote(&obj.storage_group));
        }
        if !obj.count_label.is_empty() {
            let _ = writeln!(o, "count_label = {}", quote(&obj.count_label));
        }
        if !obj.description.is_empty() {
            let _ = writeln!(o, "description = {}", quote_text(&obj.description));
        }
        if obj.kind == ObjectKind::Var {
            if let Some(s) = obj.subs.first() {
                for (k, v) in sub_fields(s, false, false) {
                    let _ = writeln!(o, "{k} = {v}");
                }
            }
            continue;
        }
        let _ = writeln!(o, "sub = [");
        for s in &obj.subs {
            let fields: Vec<String> = sub_fields(s, true, true)
                .into_iter()
                .map(|(k, v)| format!("{k} = {v}"))
                .collect();
            let _ = writeln!(o, "  {{ {} }},", fields.join(", "));
        }
        let _ = writeln!(o, "]");
        if obj.subs.iter().any(|s| !s.description.is_empty()) {
            let _ = writeln!(o, "\n[object.0x{index:04X}.doc]");
            for s in obj.subs.iter().filter(|s| !s.description.is_empty()) {
                let _ = writeln!(o, "0x{:02X} = {}", s.sub, quote_text(&s.description));
            }
        }
    }
    o
}

// ---------------------------------------------------------------- reading

fn get_str(t: &Table, key: &str) -> String {
    t.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn get_int(t: &Table, key: &str) -> Option<i64> {
    t.get(key).and_then(Value::as_integer)
}

fn get_bool(t: &Table, key: &str) -> bool {
    t.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Defaults/limits may be written as TOML strings or bare numbers.
fn get_value(t: &Table, key: &str) -> String {
    match t.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Integer(i)) => i.to_string(),
        Some(Value::Float(f)) => f.to_string(),
        Some(Value::Boolean(b)) => (*b as u8).to_string(),
        _ => String::new(),
    }
}

fn parse_hex_key(key: &str) -> Option<u32> {
    let k = key.trim();
    match k.strip_prefix("0x").or_else(|| k.strip_prefix("0X")) {
        Some(h) => u32::from_str_radix(h, 16).ok(),
        None => k.parse().ok(),
    }
}

fn read_sub(t: &Table, sub: u8, name: String, ctx: &str) -> Result<SubObject> {
    let ty = get_str(t, "type");
    let data_type =
        DataType::from_short(&ty).ok_or_else(|| anyhow!("{ctx}: unknown type {ty:?}"))?;
    let acc = t.get("access").and_then(Value::as_str).unwrap_or("rw");
    let access = Access::from_short(acc).ok_or_else(|| anyhow!("{ctx}: unknown access {acc:?}"))?;
    let pdo = t.get("pdo").and_then(Value::as_str).unwrap_or("no");
    let pdo = PdoMapping::from_short(pdo).ok_or_else(|| anyhow!("{ctx}: unknown pdo {pdo:?}"))?;
    Ok(SubObject {
        sub,
        name,
        data_type,
        access,
        pdo,
        default: get_value(t, "default"),
        min: get_value(t, "min"),
        max: get_value(t, "max"),
        description: String::new(),
        save: get_bool(t, "save"),
    })
}

pub fn from_toml(text: &str) -> Result<ObjectDictionary> {
    let root: Table = text.parse()?;
    match get_int(&root, "format") {
        Some(FORMAT_VERSION) => {}
        Some(v) => bail!("unsupported project format {v} (this build reads {FORMAT_VERSION})"),
        None => bail!("not an object dictionary project (no `format` key)"),
    }
    let mut od = ObjectDictionary::default();
    let empty = Table::new();

    let d = root.get("device").and_then(Value::as_table).unwrap_or(&empty);
    od.device = DeviceInfo {
        vendor_name: get_str(d, "vendor_name"),
        vendor_id: get_value(d, "vendor_id"),
        product_name: get_str(d, "product_name"),
        product_id: get_value(d, "product_id"),
        description: get_str(d, "description"),
        file_version: get_value(d, "file_version"),
        created_by: get_str(d, "created_by"),
        created: get_str(d, "created"),
        baud_rates: d
            .get("baud_rates")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_integer())
                    .map(|v| v as u32)
                    .collect()
            })
            .unwrap_or_default(),
        granularity: get_int(d, "granularity").unwrap_or(8) as u8,
        rpdo_count: get_int(d, "rpdo_count").unwrap_or(0) as u16,
        tpdo_count: get_int(d, "tpdo_count").unwrap_or(0) as u16,
    };

    let c = root
        .get("canfestival")
        .and_then(Value::as_table)
        .unwrap_or(&empty);
    od.canfestival = CfSettings {
        node_name: get_str(c, "node_name"),
        node_id: get_int(c, "node_id").unwrap_or(0) as u8,
        node_type: match c.get("node_type").and_then(Value::as_str) {
            Some("master") => NodeType::Master,
            _ => NodeType::Slave,
        },
        default_string_size: get_int(c, "default_string_size").unwrap_or(10) as u32,
    };

    let objects = root.get("object").and_then(Value::as_table).unwrap_or(&empty);
    for (key, value) in objects {
        let index = parse_hex_key(key)
            .filter(|i| *i <= 0xFFFF)
            .ok_or_else(|| anyhow!("bad object index {key:?}"))? as u16;
        let ctx = format!("object 0x{index:04X}");
        let t = value
            .as_table()
            .ok_or_else(|| anyhow!("{ctx}: not a table"))?;
        let kind = get_str(t, "kind");
        let kind =
            ObjectKind::from_short(&kind).ok_or_else(|| anyhow!("{ctx}: unknown kind {kind:?}"))?;
        let mut obj = Object {
            name: get_str(t, "name"),
            kind,
            disabled: get_bool(t, "disabled"),
            callback: get_bool(t, "callback"),
            description: get_str(t, "description"),
            storage_group: get_str(t, "storage_group"),
            count_label: get_str(t, "count_label"),
            subs: Vec::new(),
        };
        if kind == ObjectKind::Var {
            obj.subs.push(read_sub(t, 0, obj.name.clone(), &ctx)?);
        } else {
            let subs = t.get("sub").and_then(Value::as_array);
            for (i, sv) in subs.into_iter().flatten().enumerate() {
                let st = sv
                    .as_table()
                    .ok_or_else(|| anyhow!("{ctx}: sub entry {i} is not a table"))?;
                let sub = get_int(st, "sub")
                    .filter(|s| (0..=255).contains(s))
                    .ok_or_else(|| anyhow!("{ctx}: sub entry {i} has no valid `sub`"))?
                    as u8;
                let sctx = format!("{ctx} sub 0x{sub:02X}");
                if obj.subs.iter().any(|s| s.sub == sub) {
                    bail!("{sctx}: defined twice");
                }
                obj.subs.push(read_sub(st, sub, get_str(st, "name"), &sctx)?);
            }
            obj.subs.sort_by_key(|s| s.sub);
            if let Some(doc) = t.get("doc").and_then(Value::as_table) {
                for (k, v) in doc {
                    let sub = parse_hex_key(k).ok_or_else(|| anyhow!("{ctx}: bad doc key {k:?}"))?;
                    if let Some(s) = obj.subs.iter_mut().find(|s| s.sub as u32 == sub) {
                        s.description = v.as_str().unwrap_or_default().to_string();
                    }
                }
            }
        }
        od.objects.insert(index, obj);
    }
    Ok(od)
}
