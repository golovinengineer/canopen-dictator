// SPDX-License-Identifier: MIT OR Apache-2.0

//! EDS export (CiA 306), laid out the way CANopenEditor writes it so the
//! output can be diffed against files produced by that tool.
//!
// TODO: EDS import.
// TODO: DCF export (ParameterValue, node id, baud rate).
// TODO: CompactSubObj, ObjFlags, boot-up flags (written as 0 for now).

use std::fmt::Write as _;

use crate::model::*;

const BAUD_RATES: [u32; 8] = [10, 20, 50, 125, 250, 500, 800, 1000];

/// `hh:mmAM` and `mm-dd-yyyy` from the project timestamp.
fn eds_time(ts: &str) -> (String, String) {
    match split_timestamp(ts) {
        Some((y, m, d, h, min, _)) => {
            let (h12, ampm) = match h {
                0 => (12, "AM"),
                1..=11 => (h, "AM"),
                12 => (12, "PM"),
                _ => (h - 12, "PM"),
            };
            (
                format!("{h12}:{min:02}{ampm}"),
                format!("{m:02}-{d:02}-{y:04}"),
            )
        }
        None => ("12:00AM".into(), "01-01-2000".into()),
    }
}

/// Vendor/product ids are free text in the project; EDS wants a number.
fn id_number(s: &str) -> String {
    let t = s.trim();
    let v = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(h) => u64::from_str_radix(h, 16).ok(),
        None => t.parse::<u64>().ok(),
    };
    v.unwrap_or(0).to_string()
}

fn write_value(o: &mut String, s: &SubObject) {
    let _ = writeln!(o, "DataType=0x{:04X}", s.data_type.cia_index());
    let _ = writeln!(o, "AccessType={}", s.access.short());
    if !s.max.is_empty() {
        let _ = writeln!(o, "HighLimit={}", s.max);
    }
    if !s.min.is_empty() {
        let _ = writeln!(o, "LowLimit={}", s.min);
    }
    if s.data_type == DataType::OctetString {
        // CANopenEditor writes the hex bytes without separators.
        let packed: String = s.default.split_whitespace().collect();
        let _ = writeln!(o, "DefaultValue={packed}");
    } else {
        let _ = writeln!(o, "DefaultValue={}", s.default);
    }
    let _ = writeln!(o, "PDOMapping={}", (s.pdo != PdoMapping::No) as u8);
}

fn write_object(o: &mut String, index: u16, obj: &Object) {
    let storage = if obj.storage_group.is_empty() {
        "RAM"
    } else {
        &obj.storage_group
    };
    let _ = writeln!(o, "[{index:04X}]");
    let _ = writeln!(o, "ParameterName={}", obj.name);
    let _ = writeln!(o, "ObjectType=0x{:X}", obj.kind.code());
    let _ = writeln!(o, ";StorageLocation={storage}");
    if obj.kind == ObjectKind::Var {
        if let Some(s) = obj.subs.first() {
            write_value(o, s);
        }
        // Written for a VAR only, as CANopenEditor does.
        if obj.flags_pdo {
            let _ = writeln!(o, ";TPDODetectCos=1");
        }
        o.push('\n');
        return;
    }
    let _ = writeln!(o, "SubNumber=0x{:X}", obj.subs.len());
    o.push('\n');
    for s in &obj.subs {
        let _ = writeln!(o, "[{index:04X}sub{:X}]", s.sub);
        let _ = writeln!(o, "ParameterName={}", s.name);
        let _ = writeln!(o, "ObjectType=0x7");
        // CANopenEditor reports the storage group on the object only.
        let _ = writeln!(o, ";StorageLocation=RAM");
        write_value(o, s);
        o.push('\n');
    }
}

fn write_group(o: &mut String, title: &str, objects: &[(u16, &Object)]) {
    let _ = writeln!(o, "[{title}]");
    let _ = writeln!(o, "SupportedObjects={}", objects.len());
    for (n, (index, _)) in objects.iter().enumerate() {
        let _ = writeln!(o, "{}=0x{index:04X}", n + 1);
    }
    o.push('\n');
    for (index, obj) in objects {
        write_object(o, *index, obj);
    }
}

pub fn generate(od: &ObjectDictionary, file_name: &str) -> String {
    let d = &od.device;
    let (time, date) = eds_time(&d.created);
    let mut o = String::new();

    let _ = writeln!(o, "[FileInfo]");
    let _ = writeln!(o, "FileName={file_name}");
    let _ = writeln!(o, "FileVersion={}", d.file_version);
    let _ = writeln!(o, "FileRevision=1");
    let _ = writeln!(o, "LastEDS=");
    let _ = writeln!(o, "EDSVersion=4.0");
    let _ = writeln!(o, "Description={}", d.description.replace('\n', " "));
    let _ = writeln!(o, "CreationTime={time}");
    let _ = writeln!(o, "CreationDate={date}");
    let _ = writeln!(o, "CreatedBy={}", d.created_by);
    // The project stores no modification time on purpose (see DeviceInfo).
    let _ = writeln!(o, "ModificationTime={time}");
    let _ = writeln!(o, "ModificationDate={date}");
    let _ = writeln!(o, "ModifiedBy=");

    let _ = writeln!(o, "\n[DeviceInfo]");
    let _ = writeln!(o, "VendorName={}", d.vendor_name);
    let _ = writeln!(o, "VendorNumber={}", id_number(&d.vendor_id));
    let _ = writeln!(o, "ProductName={}", d.product_name);
    let _ = writeln!(o, "ProductNumber={}", id_number(&d.product_id));
    let _ = writeln!(o, "RevisionNumber=0");
    for b in BAUD_RATES {
        let _ = writeln!(o, "BaudRate_{b}={}", d.baud_rates.contains(&b) as u8);
    }
    let _ = writeln!(o, "SimpleBootUpMaster=0");
    let _ = writeln!(o, "SimpleBootUpSlave=0");
    let _ = writeln!(o, "Granularity={}", d.granularity);
    let _ = writeln!(o, "DynamicChannelsSupported=0");
    let _ = writeln!(o, "CompactPDO=0");
    let _ = writeln!(o, "GroupMessaging=0");
    let _ = writeln!(o, "NrOfRXPDO={}", d.rpdo_count);
    let _ = writeln!(o, "NrOfTXPDO={}", d.tpdo_count);
    let _ = writeln!(o, "LSS_Supported={}", d.lss_slave as u8);
    let _ = writeln!(o, "NG_Slave=0");

    let _ = writeln!(o, "\n[DummyUsage]");
    for i in 1..=7 {
        let _ = writeln!(o, "Dummy{i:04}={}", d.dummy_usage.contains(&i) as u8);
    }
    let _ = writeln!(o, "\n[Comments]\nLines=0\n");

    let mut mandatory = Vec::new();
    let mut optional = Vec::new();
    let mut manufacturer = Vec::new();
    for (index, obj) in od.enabled() {
        match index {
            0x1000 | 0x1001 | 0x1018 => mandatory.push((index, obj)),
            0x2000..=0x5FFF => manufacturer.push((index, obj)),
            _ => optional.push((index, obj)),
        }
    }
    write_group(&mut o, "MandatoryObjects", &mandatory);
    write_group(&mut o, "OptionalObjects", &optional);
    write_group(&mut o, "ManufacturerObjects", &manufacturer);
    o
}
