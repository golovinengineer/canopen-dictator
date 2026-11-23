// SPDX-License-Identifier: MIT OR Apache-2.0

//! In-memory object dictionary model shared by the project file, importers,
//! exporters and the GUI.

use std::collections::BTreeMap;

/// CANopen basic data types (CiA 301 index 0x0001..0x001B).
// TODO: 24/40/48/56-bit integers, TIME_OF_DAY, TIME_DIFFERENCE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataType {
    Boolean,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Real32,
    Real64,
    VisibleString,
    OctetString,
    UnicodeString,
    Domain,
}

impl DataType {
    pub const ALL: [DataType; 15] = [
        DataType::Boolean,
        DataType::I8,
        DataType::I16,
        DataType::I32,
        DataType::I64,
        DataType::U8,
        DataType::U16,
        DataType::U32,
        DataType::U64,
        DataType::Real32,
        DataType::Real64,
        DataType::VisibleString,
        DataType::OctetString,
        DataType::UnicodeString,
        DataType::Domain,
    ];

    /// Short name used in the project file and the GUI.
    pub fn short(self) -> &'static str {
        match self {
            DataType::Boolean => "bool",
            DataType::I8 => "i8",
            DataType::I16 => "i16",
            DataType::I32 => "i32",
            DataType::I64 => "i64",
            DataType::U8 => "u8",
            DataType::U16 => "u16",
            DataType::U32 => "u32",
            DataType::U64 => "u64",
            DataType::Real32 => "f32",
            DataType::Real64 => "f64",
            DataType::VisibleString => "string",
            DataType::OctetString => "octets",
            DataType::UnicodeString => "wstring",
            DataType::Domain => "domain",
        }
    }

    pub fn from_short(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.short() == s)
    }

    /// CiA 301 data type index, as written to EDS `DataType=`.
    pub fn cia_index(self) -> u16 {
        match self {
            DataType::Boolean => 0x01,
            DataType::I8 => 0x02,
            DataType::I16 => 0x03,
            DataType::I32 => 0x04,
            DataType::U8 => 0x05,
            DataType::U16 => 0x06,
            DataType::U32 => 0x07,
            DataType::Real32 => 0x08,
            DataType::VisibleString => 0x09,
            DataType::OctetString => 0x0A,
            DataType::UnicodeString => 0x0B,
            DataType::Domain => 0x0F,
            DataType::Real64 => 0x11,
            DataType::I64 => 0x15,
            DataType::U64 => 0x1B,
        }
    }

    pub fn from_cia_index(i: u16) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.cia_index() == i)
    }

    /// CiA 301 name (also the CanFestival type name).
    pub fn cia_name(self) -> &'static str {
        match self {
            DataType::Boolean => "BOOLEAN",
            DataType::I8 => "INTEGER8",
            DataType::I16 => "INTEGER16",
            DataType::I32 => "INTEGER32",
            DataType::I64 => "INTEGER64",
            DataType::U8 => "UNSIGNED8",
            DataType::U16 => "UNSIGNED16",
            DataType::U32 => "UNSIGNED32",
            DataType::U64 => "UNSIGNED64",
            DataType::Real32 => "REAL32",
            DataType::Real64 => "REAL64",
            DataType::VisibleString => "VISIBLE_STRING",
            DataType::OctetString => "OCTET_STRING",
            DataType::UnicodeString => "UNICODE_STRING",
            DataType::Domain => "DOMAIN",
        }
    }

    pub fn is_string(self) -> bool {
        matches!(
            self,
            DataType::VisibleString | DataType::OctetString | DataType::UnicodeString
        )
    }

    /// Smallest and largest value of an integer type (BOOLEAN is 0..1);
    /// `None` for reals, strings and domains.
    pub fn int_range(self) -> Option<(i128, i128)> {
        Some(match self {
            DataType::Boolean => (0, 1),
            DataType::I8 => (i8::MIN.into(), i8::MAX.into()),
            DataType::I16 => (i16::MIN.into(), i16::MAX.into()),
            DataType::I32 => (i32::MIN.into(), i32::MAX.into()),
            DataType::I64 => (i64::MIN.into(), i64::MAX.into()),
            DataType::U8 => (0, u8::MAX.into()),
            DataType::U16 => (0, u16::MAX.into()),
            DataType::U32 => (0, u32::MAX.into()),
            DataType::U64 => (0, u64::MAX.into()),
            _ => return None,
        })
    }
}

/// SDO access. `Rwr`/`Rww` are read-write with a process input/output hint
/// (EDS `rwr`/`rww`), as used by CANopenEditor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Access {
    Ro,
    Wo,
    Rw,
    Rwr,
    Rww,
    Const,
}

impl Access {
    pub const ALL: [Access; 6] = [
        Access::Ro,
        Access::Wo,
        Access::Rw,
        Access::Rwr,
        Access::Rww,
        Access::Const,
    ];

    pub fn short(self) -> &'static str {
        match self {
            Access::Ro => "ro",
            Access::Wo => "wo",
            Access::Rw => "rw",
            Access::Rwr => "rwr",
            Access::Rww => "rww",
            Access::Const => "const",
        }
    }

    pub fn from_short(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.short() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PdoMapping {
    No,
    Optional,
    Tpdo,
    Rpdo,
}

impl PdoMapping {
    pub const ALL: [PdoMapping; 4] = [
        PdoMapping::No,
        PdoMapping::Optional,
        PdoMapping::Tpdo,
        PdoMapping::Rpdo,
    ];

    pub fn short(self) -> &'static str {
        match self {
            PdoMapping::No => "no",
            PdoMapping::Optional => "optional",
            PdoMapping::Tpdo => "tpdo",
            PdoMapping::Rpdo => "rpdo",
        }
    }

    pub fn from_short(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.short() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Var,
    Array,
    Record,
}

impl ObjectKind {
    pub const ALL: [ObjectKind; 3] = [ObjectKind::Var, ObjectKind::Array, ObjectKind::Record];

    pub fn short(self) -> &'static str {
        match self {
            ObjectKind::Var => "var",
            ObjectKind::Array => "array",
            ObjectKind::Record => "record",
        }
    }

    pub fn from_short(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.short() == s)
    }

    /// CANopen object type code (EDS `ObjectType=`).
    pub fn code(self) -> u8 {
        match self {
            ObjectKind::Var => 7,
            ObjectKind::Array => 8,
            ObjectKind::Record => 9,
        }
    }
}

/// One sub-object. A `Var` object has exactly one of these, with `sub == 0`.
#[derive(Debug, Clone, PartialEq)]
pub struct SubObject {
    pub sub: u8,
    pub name: String,
    pub data_type: DataType,
    pub access: Access,
    pub pdo: PdoMapping,
    /// Kept as written (`0x22000508`, `$NODEID+0x180`, `Example Device`); empty = unset.
    pub default: String,
    pub min: String,
    pub max: String,
    pub description: String,
    /// CanFestival `TO_BE_SAVE` flag.
    pub save: bool,
    /// CANopenNode `CO_stringLengthMin`: room reserved for a string, in
    /// characters; 0 = the length of the default.
    pub string_length_min: u32,
}

impl SubObject {
    pub fn new(sub: u8, name: &str, data_type: DataType, access: Access) -> Self {
        SubObject {
            sub,
            name: name.to_string(),
            data_type,
            access,
            pdo: PdoMapping::No,
            default: String::new(),
            min: String::new(),
            max: String::new(),
            description: String::new(),
            save: false,
            string_length_min: 0,
        }
    }
}

impl SubObject {
    /// The default, minimum and maximum that won't export for this
    /// subindex's type, as (field, value as written, reason), with field
    /// `"default"`, `"minimum"` or `"maximum"`.
    pub fn bad_values(&self) -> Vec<(&'static str, &str, String)> {
        [
            ("default", &self.default),
            ("minimum", &self.min),
            ("maximum", &self.max),
        ]
        .into_iter()
        .filter_map(|(field, v)| {
            check_value(v, self.data_type)
                .err()
                .map(|why| (field, v.trim(), why))
        })
        .collect()
    }

    /// A default outside the minimum/maximum, or a minimum above the
    /// maximum; see [`check_limits`].
    pub fn limit_problems(&self) -> Vec<(&'static str, String)> {
        check_limits(&self.default, &self.min, &self.max, self.data_type)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub name: String,
    pub kind: ObjectKind,
    /// Kept in the project but left out of every export.
    pub disabled: bool,
    /// CanFestival: generate an `ODCallback_t` array for this object.
    pub callback: bool,
    pub description: String,
    /// CANopenNode storage group (informational; EDS `;StorageLocation`).
    pub storage_group: String,
    /// CANopenNode count label (informational; shown in docs).
    pub count_label: String,
    /// CANopenNode `CO_flagsPDO`: a change of a mapped value triggers the
    /// TPDO (EDS `;TPDODetectCos=1`).
    pub flags_pdo: bool,
    pub subs: Vec<SubObject>,
}

impl Object {
    pub fn new(name: &str, kind: ObjectKind) -> Self {
        let subs = match kind {
            ObjectKind::Var => vec![SubObject::new(0, name, DataType::U32, Access::Rw)],
            _ => vec![{
                let mut s =
                    SubObject::new(0, "Highest sub-index supported", DataType::U8, Access::Ro);
                s.default = "0".into();
                s
            }],
        };
        Object {
            name: name.to_string(),
            kind,
            disabled: false,
            callback: false,
            description: String::new(),
            storage_group: String::new(),
            count_label: String::new(),
            flags_pdo: false,
            subs,
        }
    }

    pub fn sub(&self, sub: u8) -> Option<&SubObject> {
        self.subs.iter().find(|s| s.sub == sub)
    }

    pub fn max_sub(&self) -> u8 {
        self.subs.iter().map(|s| s.sub).max().unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeviceInfo {
    pub vendor_name: String,
    pub vendor_id: String,
    pub product_name: String,
    pub product_id: String,
    pub description: String,
    pub file_version: String,
    pub created_by: String,
    /// Creation timestamp, `YYYY-MM-DDTHH:MM:SS`. Exports use it for both the
    /// creation and modification dates so that re-exporting an unchanged
    /// project produces identical files.
    pub created: String,
    /// Supported baud rates in kbit/s.
    pub baud_rates: Vec<u32>,
    pub granularity: u8,
    pub rpdo_count: u16,
    pub tpdo_count: u16,
    pub lss_slave: bool,
    pub lss_master: bool,
    /// Data type indexes (1..=7) that may be mapped as PDO dummy entries
    /// (EDS `[DummyUsage]`).
    pub dummy_usage: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Slave,
    Master,
}

impl NodeType {
    pub fn short(self) -> &'static str {
        match self {
            NodeType::Slave => "slave",
            NodeType::Master => "master",
        }
    }
}

/// Settings only the CanFestival C generator needs.
#[derive(Debug, Clone, PartialEq)]
pub struct CfSettings {
    /// Prefix of every generated symbol (`<name>_Data`, `<name>_obj1000`).
    pub node_name: String,
    pub node_id: u8,
    pub node_type: NodeType,
    pub default_string_size: u32,
}

impl Default for CfSettings {
    fn default() -> Self {
        CfSettings {
            node_name: String::new(),
            node_id: 0,
            node_type: NodeType::Slave,
            default_string_size: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ObjectDictionary {
    pub device: DeviceInfo,
    pub canfestival: CfSettings,
    pub objects: BTreeMap<u16, Object>,
}

impl ObjectDictionary {
    /// A new project with the three mandatory objects.
    pub fn new_default() -> Self {
        let mut od = ObjectDictionary::default();
        od.device.granularity = 8;
        od.device.baud_rates = vec![1000];
        od.device.file_version = "1".into();
        od.device.created = now_timestamp();

        let mut o = Object::new("Device type", ObjectKind::Var);
        o.subs[0].access = Access::Ro;
        o.subs[0].default = "0x00000000".into();
        od.objects.insert(0x1000, o);

        let mut o = Object::new("Error register", ObjectKind::Var);
        o.subs[0].data_type = DataType::U8;
        o.subs[0].access = Access::Ro;
        o.subs[0].default = "0x00".into();
        od.objects.insert(0x1001, o);

        let mut o = Object::new("Identity", ObjectKind::Record);
        o.subs[0].default = "0x04".into();
        for (i, n) in [
            "Vendor-ID",
            "Product code",
            "Revision number",
            "Serial number",
        ]
        .iter()
        .enumerate()
        {
            let mut s = SubObject::new(i as u8 + 1, n, DataType::U32, Access::Ro);
            s.default = "0x00000000".into();
            o.subs.push(s);
        }
        od.objects.insert(0x1018, o);
        od
    }

    /// Objects that take part in exports, in index order.
    pub fn enabled(&self) -> impl Iterator<Item = (u16, &Object)> {
        self.objects
            .iter()
            .filter(|(_, o)| !o.disabled)
            .map(|(i, o)| (*i, o))
    }

    /// Every default, minimum and maximum of an enabled object that
    /// [`check_value`] refuses, as `0x1001: default 300 does not fit
    /// UNSIGNED8` (`0x1018 sub 0x02: ...` for an ARRAY or RECORD), and
    /// every default outside its limits or minimum above its maximum
    /// ([`check_limits`]). Exports
    /// refuse to run while there are any; imports report them as warnings.
    /// Disabled objects are left out, as they are left out of every export.
    pub fn value_problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for (index, obj) in self.enabled() {
            for s in &obj.subs {
                let at = if obj.kind == ObjectKind::Var {
                    format!("0x{index:04X}")
                } else {
                    format!("0x{index:04X} sub 0x{:02X}", s.sub)
                };
                let type_problems = s
                    .bad_values()
                    .into_iter()
                    .map(|(field, _, why)| (field, why));
                for (field, why) in type_problems.chain(s.limit_problems()) {
                    problems.push(format!("{at}: {field} {why}"));
                }
            }
        }
        problems
    }
}

/// Parse an integer written as decimal, `0x` hex or with a leading `-`.
pub fn parse_int(s: &str) -> Option<i128> {
    let s = s.trim();
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s),
    };
    let v = if let Some(h) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i128::from_str_radix(h, 16).ok()?
    } else {
        body.parse::<i128>().ok()?
    };
    Some(if neg { -v } else { v })
}

/// Check a default, minimum or maximum written for a value of type `t`, in
/// the forms the exporters accept: empty (unset), any text for strings and
/// domains, a decimal number for reals, and for integers `TRUE`/`FALSE`,
/// `$NODEID` or `$NODEID+offset`, or a decimal or `0x` hex number that fits
/// the type.
pub fn check_value(text: &str, t: DataType) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() || t.is_string() || t == DataType::Domain {
        return Ok(());
    }
    let Some((min, max)) = t.int_range() else {
        return text
            .parse::<f64>()
            .map(|_| ())
            .map_err(|_| format!("{text:?} is not a number"));
    };
    let upper = text.to_uppercase();
    if upper == "TRUE" || upper == "FALSE" {
        return Ok(());
    }
    if let Some(rest) = upper.strip_prefix("$NODEID") {
        let rest = rest.trim();
        let ok = rest.is_empty() || rest.strip_prefix('+').and_then(parse_int).is_some();
        return if ok {
            Ok(())
        } else {
            Err(format!("{text:?} is not a $NODEID formula"))
        };
    }
    let v = parse_int(text).ok_or_else(|| format!("{text:?} is not a number"))?;
    if v < min || v > max {
        return Err(format!("{text} does not fit {}", t.cia_name()));
    }
    Ok(())
}

/// Check a default against its minimum and maximum, and the minimum
/// against the maximum, for values of type `t` that each pass
/// [`check_value`]. Returns (field, reason) with field `"default"` or
/// `"minimum"`, e.g. `("default", "300 is above the maximum 255")`.
/// Only numbers are compared: empty values, strings, domains and
/// `$NODEID` formulas (their value depends on the node id) are skipped,
/// and so are values [`check_value`] refuses, which are reported as such.
pub fn check_limits(
    default: &str,
    min: &str,
    max: &str,
    t: DataType,
) -> Vec<(&'static str, String)> {
    fn compare<T: PartialOrd>(
        parse: impl Fn(&str) -> Option<T>,
        [default, min, max]: [&str; 3],
    ) -> Vec<(&'static str, String)> {
        let (d, lo, hi) = (parse(default), parse(min), parse(max));
        if let (Some(lo), Some(hi)) = (&lo, &hi)
            && lo > hi
        {
            // The default can't be judged against a range that is upside down.
            return vec![("minimum", format!("{min} is above the maximum {max}"))];
        }
        let mut problems = Vec::new();
        if let (Some(d), Some(lo)) = (&d, &lo)
            && d < lo
        {
            problems.push(("default", format!("{default} is below the minimum {min}")));
        }
        if let (Some(d), Some(hi)) = (&d, &hi)
            && d > hi
        {
            problems.push(("default", format!("{default} is above the maximum {max}")));
        }
        problems
    }
    let texts = [default.trim(), min.trim(), max.trim()];
    let usable = |text: &str| !text.is_empty() && check_value(text, t).is_ok();
    if t.int_range().is_some() {
        compare(
            |text| {
                let upper = text.to_uppercase();
                match upper.as_str() {
                    _ if !usable(text) => None,
                    "TRUE" => Some(1),
                    "FALSE" => Some(0),
                    _ => parse_int(text),
                }
            },
            texts,
        )
    } else if matches!(t, DataType::Real32 | DataType::Real64) {
        compare(
            |text| usable(text).then(|| text.parse::<f64>().ok()).flatten(),
            texts,
        )
    } else {
        Vec::new()
    }
}

/// Current UTC time as `YYYY-MM-DDTHH:MM:SS`.
pub fn now_timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Split a `YYYY-MM-DDTHH:MM:SS` timestamp into (y, m, d, h, min, s).
pub fn split_timestamp(ts: &str) -> Option<(u32, u32, u32, u32, u32, u32)> {
    let (date, time) = ts.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<u32>().ok());
    let mut t = time.split(':').map(|p| p.parse::<u32>().ok());
    Some((
        d.next()??,
        d.next()??,
        d.next()??,
        t.next()??,
        t.next()??,
        t.next().flatten().unwrap_or(0),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_value_accepts_exporter_forms() {
        for (text, t) in [
            ("", DataType::U8),
            ("0x10", DataType::U8),
            ("255", DataType::U8),
            ("-128", DataType::I8),
            ("$NODEID+0x180", DataType::U32),
            ("$nodeid + 0x180", DataType::U32),
            ("$NODEID", DataType::U8),
            ("TRUE", DataType::Boolean),
            ("1", DataType::Boolean),
            ("1.5e3", DataType::Real32),
            ("any text", DataType::VisibleString),
            ("0xFFFFFFFFFFFFFFFF", DataType::U64),
        ] {
            assert_eq!(check_value(text, t), Ok(()), "{text:?} as {t:?}");
        }
    }

    #[test]
    fn check_limits_compares_numbers_only() {
        use DataType::*;
        let none: Vec<(&str, String)> = Vec::new();
        assert_eq!(
            check_limits("300", "", "255", U16),
            [("default", "300 is above the maximum 255".to_string())]
        );
        assert_eq!(
            check_limits("0x05", "10", "", U8),
            [("default", "0x05 is below the minimum 10".to_string())]
        );
        assert_eq!(
            check_limits("-1.5", "-1", "1", Real32),
            [("default", "-1.5 is below the minimum -1".to_string())]
        );
        // An upside-down range is reported once, without judging the default.
        assert_eq!(
            check_limits("7", "10", "5", I16),
            [("minimum", "10 is above the maximum 5".to_string())]
        );
        // Hex, decimal and booleans compare by value.
        assert_eq!(check_limits("0x10", "16", "0x10", U8), none);
        assert_eq!(check_limits("TRUE", "0", "1", Boolean), none);
        // Skipped: unset limits, $NODEID formulas, strings, values that don't fit the type.
        assert_eq!(check_limits("300", "", "", U16), none);
        assert_eq!(check_limits("$NODEID+0x180", "0", "0x100", U32), none);
        assert_eq!(check_limits("zz", "a", "b", VisibleString), none);
        assert_eq!(check_limits("300", "0", "10", U8), none);
    }

    #[test]
    fn check_value_rejects_bad_text_and_range() {
        for (text, t) in [
            ("bla", DataType::U8),
            ("256", DataType::U8),
            ("-1", DataType::U16),
            ("2", DataType::Boolean),
            ("0x", DataType::U32),
            ("$NODEID-1", DataType::U32),
            ("$NODEID+x", DataType::U32),
            ("1,5", DataType::Real64),
        ] {
            assert!(check_value(text, t).is_err(), "{text:?} as {t:?}");
        }
    }
}
