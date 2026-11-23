// SPDX-License-Identifier: MIT OR Apache-2.0

//! Import of CANopen XDD device descriptions (CiA 311), as written by
//! CANopenEditor / libedssharp.
//!
//! The XDD splits every object across two lists: `CANopenObjectList` (index,
//! name, object type, PDO mapping) and `parameterList` (data type, access,
//! default, limits, description, `CO_*` properties), joined by `uniqueIDRef`.
//!
// TODO: XDD export (backlog).
// TODO: XDC/XPD variants (actualValue, node id, parameter groups).

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use roxmltree::{Document, Node};

use crate::model::*;

pub fn load(path: &Path) -> Result<ObjectDictionary> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse(&text).with_context(|| format!("parsing {}", path.display()))
}

#[derive(Default)]
struct Param {
    data_type: Option<DataType>,
    access: Option<Access>,
    default: String,
    min: String,
    max: String,
    description: String,
    props: HashMap<String, String>,
}

fn xdd_type(tag: &str) -> Option<DataType> {
    Some(match tag {
        "BOOL" => DataType::Boolean,
        "SINT" => DataType::I8,
        "INT" => DataType::I16,
        "DINT" => DataType::I32,
        "LINT" => DataType::I64,
        "USINT" | "BYTE" | "CHAR" => DataType::U8,
        "UINT" | "WORD" => DataType::U16,
        "UDINT" | "DWORD" => DataType::U32,
        "ULINT" | "LWORD" => DataType::U64,
        "REAL" => DataType::Real32,
        "LREAL" => DataType::Real64,
        "STRING" => DataType::VisibleString,
        "WSTRING" => DataType::UnicodeString,
        // CANopenEditor writes OCTET_STRING and DOMAIN alike as BITSTRING;
        // `parse_param` tells them apart by the default, as it does.
        "BITSTRING" => DataType::OctetString,
        _ => return None,
    })
}

fn xdd_access(s: &str) -> Access {
    match s {
        "const" => Access::Const,
        "write" => Access::Wo,
        "readWrite" => Access::Rw,
        "readWriteInput" => Access::Rwr,
        "readWriteOutput" => Access::Rww,
        _ => Access::Ro, // "read", "noAccess"
    }
}

fn xdd_pdo(s: &str) -> PdoMapping {
    match s {
        "optional" | "default" => PdoMapping::Optional,
        "TPDO" => PdoMapping::Tpdo,
        "RPDO" => PdoMapping::Rpdo,
        _ => PdoMapping::No,
    }
}

fn children<'a>(n: Node<'a, 'a>, name: &'static str) -> impl Iterator<Item = Node<'a, 'a>> {
    n.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

fn child<'a>(n: Node<'a, 'a>, name: &'static str) -> Option<Node<'a, 'a>> {
    children(n, name).next()
}

fn find<'a>(doc: &'a Document<'a>, name: &str) -> Option<Node<'a, 'a>> {
    doc.descendants()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}

fn text(n: Node) -> String {
    n.text().unwrap_or("").replace("\r\n", "\n")
}

fn parse_param(n: Node) -> Param {
    let mut p = Param {
        access: n.attribute("access").map(xdd_access),
        ..Default::default()
    };
    for c in n.children().filter(|c| c.is_element()) {
        match c.tag_name().name() {
            "description" => p.description = text(c),
            "label" | "dataTypeIDRef" => {}
            "defaultValue" => p.default = c.attribute("value").unwrap_or("").to_string(),
            "allowedValues" => {
                if let Some(r) = child(c, "range") {
                    if let Some(v) = child(r, "minValue") {
                        p.min = v.attribute("value").unwrap_or("").to_string();
                    }
                    if let Some(v) = child(r, "maxValue") {
                        p.max = v.attribute("value").unwrap_or("").to_string();
                    }
                }
            }
            "property" => {
                if let (Some(k), Some(v)) = (c.attribute("name"), c.attribute("value")) {
                    p.props.insert(k.to_string(), v.to_string());
                }
            }
            tag => {
                if let Some(t) = xdd_type(tag) {
                    p.data_type = Some(t);
                }
            }
        }
    }
    // libEDSsharp: BITSTRING with a default is an OCTET_STRING (hex bytes),
    // without one a DOMAIN.
    if p.data_type == Some(DataType::OctetString) && p.default.is_empty() {
        p.data_type = Some(DataType::Domain);
    }
    p
}

pub fn parse(xml: &str) -> Result<ObjectDictionary> {
    let doc = Document::parse(xml)?;
    let mut od = ObjectDictionary::default();

    // --- device identity / file info -------------------------------------
    if let Some(id) = find(&doc, "DeviceIdentity") {
        let get = |name| child(id, name).map(text).unwrap_or_default();
        od.device.vendor_name = get("vendorName");
        od.device.vendor_id = get("vendorID");
        od.device.product_name = get("productName");
        od.device.product_id = get("productID");
        if let Some(pt) = child(id, "productText") {
            od.device.description = child(pt, "description").map(text).unwrap_or_default();
        }
    }
    if let Some(body) = find(&doc, "ProfileBody") {
        od.device.file_version = body.attribute("fileVersion").unwrap_or("1").to_string();
        od.device.created_by = body.attribute("fileCreator").unwrap_or("").to_string();
        // fileCreationDate="2026-05-06" fileCreationTime="11:40:09.0000000+03:00"
        let date = body.attribute("fileCreationDate").unwrap_or("");
        let time = body.attribute("fileCreationTime").unwrap_or("");
        if date.len() == 10 && time.len() >= 8 {
            od.device.created = format!("{date}T{}", &time[..8]);
        }
    }
    for b in doc
        .descendants()
        .filter(|n| n.tag_name().name() == "supportedBaudRate")
    {
        // "1000 Kbps"
        if let Some(v) = b
            .attribute("value")
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse().ok())
        {
            od.device.baud_rates.push(v);
        }
    }
    od.device.granularity = 8;
    if let Some(f) = find(&doc, "CANopenGeneralFeatures") {
        let num = |name: &str| f.attribute(name).and_then(|v| v.parse::<u16>().ok());
        od.device.granularity = num("granularity").unwrap_or(8) as u8;
        od.device.rpdo_count = num("nrOfRxPDO").unwrap_or(0);
        od.device.tpdo_count = num("nrOfTxPDO").unwrap_or(0);
        od.device.lss_slave = f.attribute("layerSettingServiceSlave") == Some("true");
    }
    if let Some(f) = find(&doc, "CANopenMasterFeatures") {
        od.device.lss_master = f.attribute("layerSettingServiceMaster") == Some("true");
    }
    // <dummy entry="Dummy0005=1" />
    for d in doc.descendants().filter(|n| n.tag_name().name() == "dummy") {
        if let Some((name, on)) = d.attribute("entry").and_then(|e| e.split_once('='))
            && on.trim() == "1"
            && let Some(t) = name
                .trim()
                .strip_prefix("Dummy")
                .and_then(|t| t.parse().ok())
        {
            od.device.dummy_usage.push(t);
        }
    }

    // --- parameters by unique id -----------------------------------------
    let mut params: HashMap<&str, Param> = HashMap::new();
    if let Some(list) = find(&doc, "parameterList") {
        for p in children(list, "parameter") {
            if let Some(uid) = p.attribute("uniqueID") {
                params.insert(uid, parse_param(p));
            }
        }
    }

    // --- objects ------------------------------------------------------------
    let list = find(&doc, "CANopenObjectList").ok_or_else(|| anyhow!("no CANopenObjectList"))?;
    for o in children(list, "CANopenObject") {
        let index = o
            .attribute("index")
            .and_then(|s| u16::from_str_radix(s, 16).ok())
            .ok_or_else(|| anyhow!("CANopenObject without a valid index"))?;
        let name = o.attribute("name").unwrap_or("").to_string();
        let kind = match o.attribute("objectType").unwrap_or("7") {
            "7" | "2" | "5" | "6" => ObjectKind::Var,
            "8" => ObjectKind::Array,
            "9" => ObjectKind::Record,
            other => bail!("object 0x{index:04X}: unsupported objectType {other}"),
        };
        let empty = Param::default();
        let op = o
            .attribute("uniqueIDRef")
            .and_then(|u| params.get(u))
            .unwrap_or(&empty);

        let make_sub = |sub: u8, n: Node, p: &Param| -> SubObject {
            SubObject {
                sub,
                name: n.attribute("name").unwrap_or("").to_string(),
                // Objects without a parameter carry type/access as attributes.
                data_type: p
                    .data_type
                    .or_else(|| {
                        n.attribute("dataType")
                            .and_then(|s| u16::from_str_radix(s, 16).ok())
                            .and_then(DataType::from_cia_index)
                    })
                    .unwrap_or(DataType::U8),
                access: p
                    .access
                    .or_else(|| {
                        n.attribute("accessType")
                            .and_then(|a| Access::from_short(&a.to_lowercase()))
                    })
                    .unwrap_or(Access::Ro),
                pdo: xdd_pdo(n.attribute("PDOmapping").unwrap_or("no")),
                default: if p.default.is_empty() {
                    n.attribute("defaultValue").unwrap_or("").to_string()
                } else {
                    p.default.clone()
                },
                min: p.min.clone(),
                max: p.max.clone(),
                description: String::new(),
                save: false,
                string_length_min: p
                    .props
                    .get("CO_stringLengthMin")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0),
            }
        };

        let mut obj = Object {
            name,
            kind,
            disabled: op.props.get("CO_disabled").is_some_and(|v| v == "true"),
            callback: false,
            description: op.description.clone(),
            storage_group: op.props.get("CO_storageGroup").cloned().unwrap_or_default(),
            count_label: op.props.get("CO_countLabel").cloned().unwrap_or_default(),
            flags_pdo: op.props.get("CO_flagsPDO").is_some_and(|v| v == "true"),
            subs: Vec::new(),
        };

        if kind == ObjectKind::Var {
            obj.subs.push(make_sub(0, o, op));
        } else {
            for s in children(o, "CANopenSubObject") {
                let sub = s
                    .attribute("subIndex")
                    .and_then(|v| u8::from_str_radix(v, 16).ok())
                    .ok_or_else(|| anyhow!("object 0x{index:04X}: bad subIndex"))?;
                let sp = s
                    .attribute("uniqueIDRef")
                    .and_then(|u| params.get(u))
                    .unwrap_or(&empty);
                let mut so = make_sub(sub, s, sp);
                so.description = sp.description.clone();
                obj.subs.push(so);
            }
            obj.subs.sort_by_key(|s| s.sub);
        }
        od.objects.insert(index, obj);
    }
    Ok(od)
}
