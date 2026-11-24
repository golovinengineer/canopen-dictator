// SPDX-License-Identifier: MIT OR Apache-2.0

//! Markdown documentation export, in the layout of CANopenEditor's
//! "documentation" export so existing documents keep their shape.
//!
// TODO: SRDO column is always "no" (no SRDO support in the model).
// TODO: node-guarding flags are always False.

use std::fmt::Write as _;

use crate::model::*;

const BAUD_RATES: [u32; 8] = [10, 20, 50, 125, 250, 500, 800, 1000];

/// `M/D/YYYY h:mm:ss AM` from the project timestamp.
fn doc_time(ts: &str) -> String {
    match split_timestamp(ts) {
        Some((y, m, d, h, min, s)) => {
            let (h12, ampm) = match h {
                0 => (12, "AM"),
                1..=11 => (h, "AM"),
                12 => (12, "PM"),
                _ => (h - 12, "PM"),
            };
            format!("{m}/{d}/{y} {h12}:{min:02}:{s:02} {ampm}")
        }
        None => String::new(),
    }
}

fn sdo(a: Access) -> &'static str {
    match a {
        Access::Ro | Access::Const => "ro",
        Access::Wo => "wo",
        Access::Rw | Access::Rwr | Access::Rww => "rw",
    }
}

fn pdo(p: PdoMapping) -> &'static str {
    match p {
        PdoMapping::No => "no",
        PdoMapping::Optional => "tr",
        PdoMapping::Tpdo => "t",
        PdoMapping::Rpdo => "r",
    }
}

/// .NET's `bool.ToString()`.
fn py_bool(b: bool) -> String {
    (if b { "True" } else { "False" }).to_string()
}

/// String length suffix CANopenEditor adds to the type: ` (len=16)`.
fn len_suffix(s: &SubObject) -> String {
    if s.string_length_min != 0 {
        format!("(len={})", s.string_length_min)
    } else {
        String::new()
    }
}

fn kv_table(o: &mut String, rows: &[(&str, String)]) {
    let _ = writeln!(o, "| {:<12} | {:<30} |", "", "");
    let _ = writeln!(o, "| {} | {} |", "-".repeat(12), "-".repeat(30));
    for (k, v) in rows {
        let _ = writeln!(o, "| {k:<12} | {v:<30} |");
    }
}

/// Name shown for a mapped variable: `Object (Sub)`.
fn mapped_name(od: &ObjectDictionary, index: u16, sub: u8) -> String {
    // Dummy entries map a data type index (0x0001..0x0007).
    if index < 0x1000 {
        return DataType::from_cia_index(index)
            .map(|t| t.cia_name().to_string())
            .unwrap_or_default();
    }
    match od.objects.get(&index) {
        Some(obj) if obj.kind == ObjectKind::Var => obj.name.clone(),
        Some(obj) => match obj.sub(sub) {
            Some(s) => format!("{} ({})", obj.name, s.name),
            None => obj.name.clone(),
        },
        None => String::new(),
    }
}

fn pdo_section(o: &mut String, od: &ObjectDictionary) {
    for (comm_base, label) in [(0x1400u16, "RPDO"), (0x1800u16, "TPDO")] {
        for (index, comm) in od
            .enabled()
            .filter(|(i, _)| (comm_base..comm_base + 0x200).contains(i))
        {
            let Some(map) = od.objects.get(&(index + 0x200)).filter(|m| !m.disabled) else {
                continue;
            };
            let value =
                |obj: &Object, sub: u8| obj.sub(sub).map(|s| s.default.clone()).unwrap_or_default();
            let count = parse_int(&value(map, 0)).unwrap_or(0);
            if count <= 0 {
                continue;
            }
            let _ = writeln!(o, "\n### {label} 0x{index:04X}");
            let _ = writeln!(o, "| {:<12} | {:<61} |", "", "");
            let _ = writeln!(o, "| {} | {} |", "-".repeat(12), "-".repeat(61));
            let _ = writeln!(o, "| {:<12} | {:<61} |", "COB_ID", value(comm, 1));
            let transmission = if label == "TPDO" {
                format!(
                    "type={}; inhibit-time={}; event-timer={}",
                    value(comm, 2),
                    value(comm, 3),
                    value(comm, 5)
                )
            } else {
                format!("type={}", value(comm, 2))
            };
            let _ = writeln!(o, "| {:<12} | {transmission:<61} |", "Transmission");
            for sub in 1..=count.min(255) as u8 {
                let Some(m) = parse_int(&value(map, sub)).filter(|m| *m > 0) else {
                    continue;
                };
                let name = mapped_name(od, (m >> 16) as u16, (m >> 8) as u8);
                let _ = writeln!(o, "| {:>12} | {name:<61} |", format!("0x{m:08X}"));
            }
            o.push('\n');
        }
    }
}

fn object_section(o: &mut String, index: u16, obj: &Object) {
    let _ = writeln!(o, "\n### 0x{index:04X} - {}", obj.name);
    let _ = writeln!(o, "| Object Type | Count Label    | Storage Group  |");
    let _ = writeln!(o, "| ----------- | -------------- | -------------- |");
    let storage = if obj.storage_group.is_empty() {
        "RAM"
    } else {
        &obj.storage_group
    };
    let _ = writeln!(
        o,
        "| {:<11} | {:<14} | {storage:<14} |",
        obj.kind.short().to_uppercase(),
        obj.count_label
    );
    o.push('\n');
    if obj.kind == ObjectKind::Var {
        let _ = writeln!(
            o,
            "| Data Type               | SDO | PDO | SRDO | Default Value                   |"
        );
        let _ = writeln!(
            o,
            "| ----------------------- | --- | --- | ---- | ------------------------------- |"
        );
        if let Some(s) = obj.subs.first() {
            let mut ty = s.data_type.cia_name().to_string();
            if s.string_length_min != 0 {
                ty = format!("{ty} {}", len_suffix(s));
            }
            let _ = writeln!(
                o,
                "| {:<23} | {:<3} | {:<3} | {:<4} | {:<31} |",
                ty,
                sdo(s.access),
                pdo(s.pdo),
                "no",
                s.default
            );
        }
    } else {
        let _ = writeln!(
            o,
            "| Sub  | Name                  | Data Type  | SDO | PDO | SRDO | Default Value |"
        );
        let _ = writeln!(
            o,
            "| ---- | --------------------- | ---------- | --- | --- | ---- | ------------- |"
        );
        for s in &obj.subs {
            let _ = writeln!(
                o,
                "| 0x{:02X} | {:<22}| {:<10} {}| {:<3} | {:<3} | {:<4} | {:<14}|",
                s.sub,
                s.name,
                s.data_type.cia_name(),
                len_suffix(s),
                sdo(s.access),
                pdo(s.pdo),
                "no",
                s.default
            );
        }
    }
    let mut notes: Vec<&str> = Vec::new();
    if !obj.description.is_empty() {
        notes.push(&obj.description);
    }
    if obj.kind != ObjectKind::Var {
        notes.extend(
            obj.subs
                .iter()
                .map(|s| s.description.as_str())
                .filter(|d| !d.is_empty()),
        );
    }
    if !notes.is_empty() {
        let _ = writeln!(o, "\n{}", notes.join("\n"));
    }
}

pub fn generate(od: &ObjectDictionary, project_file: &str) -> String {
    let d = &od.device;
    let mut o = String::new();
    let _ = writeln!(
        o,
        "CANopen device documentation\n============================"
    );
    let _ = writeln!(o, "**{}**\n\n{}\n", d.product_name, d.description);
    let created = doc_time(&d.created);
    kv_table(
        &mut o,
        &[
            ("Project File", project_file.to_string()),
            ("File Version", d.file_version.clone()),
            ("Created", created.clone()),
            ("Created By", d.created_by.clone()),
            // No modification time is stored, see DeviceInfo::created.
            ("Modified", created),
            ("Modified By", String::new()),
        ],
    );
    let _ = writeln!(
        o,
        "\nThis file was automatically generated by Dictator\n\n[TOC]\n\n"
    );

    let _ = writeln!(o, "Device Information\n------------------");
    kv_table(
        &mut o,
        &[
            ("Vendor Name", d.vendor_name.clone()),
            ("Vendor ID", d.vendor_id.clone()),
            ("Product Name", d.product_name.clone()),
            ("Product ID", d.product_id.clone()),
            ("Granularity", d.granularity.to_string()),
            ("RPDO count", d.rpdo_count.to_string()),
            ("TPDO count", d.tpdo_count.to_string()),
            ("LSS Slave", py_bool(d.lss_slave)),
            ("LSS Master", py_bool(d.lss_master)),
            ("NG Slave", "False".into()),
            ("NG Master", "False".into()),
        ],
    );
    let _ = writeln!(o, "\n#### Supported Baud rates");
    for b in BAUD_RATES {
        let mark = if d.baud_rates.contains(&b) { "x" } else { " " };
        let _ = writeln!(o, "* [{mark}] {b} kBit/s");
    }
    let _ = writeln!(o, "* [ ] auto\n\n");

    let _ = writeln!(o, "PDO Mapping\n-----------");
    pdo_section(&mut o, od);

    let groups: [(&str, std::ops::RangeInclusive<u16>); 3] = [
        ("Communication Specific Parameters", 0x1000..=0x1FFF),
        ("Manufacturer Specific Parameters", 0x2000..=0x5FFF),
        ("Device Profile Specific Parameters", 0x6000..=0xFFFF),
    ];
    for (title, range) in groups {
        let objects: Vec<_> = od.enabled().filter(|(i, _)| range.contains(i)).collect();
        if objects.is_empty() {
            continue;
        }
        let _ = writeln!(o, "\n{title}\n{}", "-".repeat(title.len()));
        for (index, obj) in objects {
            object_section(&mut o, index, obj);
        }
    }
    o
}
