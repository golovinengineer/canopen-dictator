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
    /// Kept as written (`0x22000508`, `$NODEID+0x180`, `Example`); empty = unset.
    pub default: String,
    pub min: String,
    pub max: String,
    pub description: String,
    /// CanFestival `TO_BE_SAVE` flag.
    pub save: bool,
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
        }
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
        for (i, n) in ["Vendor-ID", "Product code", "Revision number", "Serial number"]
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
