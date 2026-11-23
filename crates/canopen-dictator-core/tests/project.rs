// SPDX-License-Identifier: MIT OR Apache-2.0

mod common;

use canopen_dictator_core::model::*;
use canopen_dictator_core::project;

#[test]
fn xdd_import_reads_sample() {
    let od = common::sample_xdd();
    assert_eq!(od.device.vendor_name, "Example Vendor");
    assert_eq!(od.device.product_name, "Example Device");
    assert_eq!(od.device.baud_rates, vec![125, 250, 500, 1000]);
    assert_eq!((od.device.rpdo_count, od.device.tpdo_count), (4, 4));
    assert_eq!(od.device.created, "2026-01-01T15:00:00");

    let count = |k| od.objects.values().filter(|o| o.kind == k).count();
    assert_eq!(od.objects.len(), 63);
    assert_eq!(count(ObjectKind::Array), 6);
    assert_eq!(count(ObjectKind::Record), 23);
    let disabled: Vec<u16> = od
        .objects
        .iter()
        .filter(|(_, o)| o.disabled)
        .map(|(i, _)| *i)
        .collect();
    assert_eq!(disabled, [0x1013, 0x1029, 0x2FFF]);

    let o = &od.objects[&0x1000];
    assert_eq!(o.kind, ObjectKind::Var);
    assert_eq!(o.subs[0].data_type, DataType::U32);
    assert_eq!(o.subs[0].access, Access::Ro);
    assert_eq!(o.subs[0].default, "0x00000000");
    assert_eq!(o.storage_group, "PERSIST_COMM");
    assert_eq!(o.count_label, "NMT");

    // CANopenNode v4 PDO parameters have holes.
    let o = &od.objects[&0x1800];
    assert_eq!(o.sub(1).unwrap().default, "$NODEID+0x180");
    assert!(o.sub(4).is_none() && o.sub(6).is_some());
    assert!(od.objects[&0x1400].sub(3).is_none());

    // One VAR per data type and access kind.
    let var = |i: u16| &od.objects[&i].subs[0];
    let types: Vec<DataType> = (0x2000..=0x200E).map(|i| var(i).data_type).collect();
    use DataType::*;
    assert_eq!(
        types,
        [
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
            Domain
        ]
    );
    assert_eq!(
        (var(0x2001).min.as_str(), var(0x2001).max.as_str()),
        ("-10", "10")
    );
    assert_eq!(var(0x2006).access, Access::Rww);
    assert_eq!(var(0x2007).access, Access::Rwr);
    assert_eq!(var(0x200F).access, Access::Const);
    assert_eq!(var(0x2010).access, Access::Wo);
    assert_eq!(var(0x2011).default, "$NODEID+0x100");
    assert_eq!(var(0x200C).default, "01 02 03 04");
    assert_eq!(var(0x2005).pdo, PdoMapping::Tpdo);
    assert_eq!(var(0x2000).pdo, PdoMapping::Rpdo);
    assert_eq!(var(0x2001).pdo, PdoMapping::Optional);

    let o = &od.objects[&0x2101];
    assert_eq!(o.kind, ObjectKind::Record);
    assert!(o.sub(4).is_none());
    assert_eq!(o.sub(6).unwrap().data_type, VisibleString);

    let o = &od.objects[&0x2102];
    assert_eq!(
        (o.storage_group.as_str(), o.count_label.as_str()),
        ("PERSIST_APP", "DEMO")
    );
    assert!(o.description.contains("<markup> & special"));
    assert!(o.sub(1).unwrap().description.contains("25 \u{b0}C"));

    // CANopenNode properties.
    assert!(od.device.lss_slave && !od.device.lss_master);
    assert_eq!(od.device.dummy_usage, [2, 3, 4, 5, 6, 7]);
    assert!(od.objects[&0x2005].flags_pdo && od.objects[&0x2101].flags_pdo);
    assert!(!od.objects[&0x2007].flags_pdo);
    assert_eq!(var(0x200B).string_length_min, 16);
    assert_eq!(od.objects[&0x2101].sub(6).unwrap().string_length_min, 12);
}

#[test]
fn toml_round_trip_is_lossless_and_stable() {
    let mut od = common::sample_xdd();
    od.objects.get_mut(&0x2101).unwrap().callback = true;
    od.objects.get_mut(&0x2102).unwrap().subs[1].save = true;

    let text = project::to_toml(&od);
    let back = project::from_toml(&text).unwrap();
    assert_eq!(od, back);
    assert_eq!(text, project::to_toml(&back));
}

#[test]
fn toml_escaping_survives_awkward_text() {
    let mut od = ObjectDictionary::new_default();
    let o = od.objects.get_mut(&0x1018).unwrap();
    o.description = "line \"one\"\nends with quote\"".into();
    o.subs[1].description = "back\\slash and \"\"\" triple\nsecond\tline".into();
    o.subs[1].name = "quote \" in name".into();
    let back = project::from_toml(&project::to_toml(&od)).unwrap();
    assert_eq!(od, back);
}

#[test]
fn checked_in_sample_project_matches_xdd() {
    let mut from_xdd = common::sample_xdd();
    // The checked-in project adds the CanFestival settings made in
    // objdictedit for example.od; the XDD has no place for them.
    from_xdd.canfestival.node_name = "ExampleDevice".into();
    for i in [0x2000, 0x2006, 0x2101, 0x2102, 0x6200] {
        from_xdd.objects.get_mut(&i).unwrap().callback = true;
    }
    from_xdd.objects.get_mut(&0x2012).unwrap().subs[0].save = true;
    for s in from_xdd
        .objects
        .get_mut(&0x2102)
        .unwrap()
        .subs
        .iter_mut()
        .filter(|s| s.sub >= 1)
    {
        s.save = true;
    }
    assert_eq!(from_xdd, common::sample_project());
}

#[test]
fn value_problems_name_each_value_that_wont_export() {
    // The example, from either source, exports as it is.
    assert_eq!(common::sample_xdd().value_problems(), Vec::<String>::new());
    assert_eq!(
        common::sample_project().value_problems(),
        Vec::<String>::new()
    );

    let mut od = common::sample_project();
    od.objects.get_mut(&0x1001).unwrap().subs[0].default = "300".into();
    let identity = od.objects.get_mut(&0x1018).unwrap();
    identity.subs[1].max = "lots".into();
    assert_eq!(
        od.value_problems(),
        [
            "0x1001: default 300 does not fit UNSIGNED8",
            "0x1018 sub 0x01: maximum \"lots\" is not a number",
        ]
    );

    // A default outside the limits, after the type problems of the same sub.
    let identity = od.objects.get_mut(&0x1018).unwrap();
    identity.subs[2].min = "0x100".into();
    identity.subs[2].default = "0x10".into();
    assert_eq!(
        od.value_problems()[2],
        "0x1018 sub 0x02: default 0x10 is below the minimum 0x100"
    );

    // Disabled objects are left out of exports, and so out of the check.
    od.objects.get_mut(&0x1018).unwrap().disabled = true;
    assert_eq!(
        od.value_problems(),
        ["0x1001: default 300 does not fit UNSIGNED8"]
    );
}
