mod common;

use dictator::model::*;
use dictator::project;

#[test]
fn xdd_import_reads_sample() {
    let od = common::sample_xdd();
    assert_eq!(od.device.vendor_name, "Example Vendor");
    assert_eq!(od.device.product_name, "Example");
    assert_eq!(od.device.baud_rates, vec![1000]);
    assert_eq!((od.device.rpdo_count, od.device.tpdo_count), (4, 4));

    // 494 VAR-typed entries minus sub-objects, 9 arrays, 56 records in the XDD.
    let count = |k| od.objects.values().filter(|o| o.kind == k).count();
    assert_eq!(count(ObjectKind::Array), 9);
    assert_eq!(count(ObjectKind::Record), 56);
    assert_eq!(od.objects.values().filter(|o| o.disabled).count(), 12);

    let o = &od.objects[&0x1000];
    assert_eq!(o.kind, ObjectKind::Var);
    assert_eq!(o.subs[0].data_type, DataType::U32);
    assert_eq!(o.subs[0].access, Access::Ro);
    assert_eq!(o.subs[0].default, "0x00000000");
    assert_eq!(o.storage_group, "PERSIST_COMM");
    assert_eq!(o.count_label, "NMT");

    let o = &od.objects[&0x1800];
    assert_eq!(o.sub(1).unwrap().default, "$NODEID+0x180");
    assert!(o.sub(4).is_none());

    let o = &od.objects[&0x2001];
    let s = o.sub(1).unwrap();
    assert_eq!((s.min.as_str(), s.max.as_str(), s.default.as_str()), ("6", "18", "18"));
    assert_eq!(s.description, "number of cells");

    let s = od.objects[&0x2200].sub(5).unwrap();
    assert_eq!(s.access, Access::Rwr);
    assert_eq!(s.pdo, PdoMapping::Tpdo);
    assert!(s.description.contains("Digital inputs 1"));
}

#[test]
fn toml_round_trip_is_lossless_and_stable() {
    let mut od = common::sample_xdd();
    od.objects.get_mut(&0x2200).unwrap().callback = true;
    od.objects.get_mut(&0x2001).unwrap().subs[1].save = true;

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
    let from_toml = project::load(&common::asset("legacy.objdict.toml")).unwrap();
    let mut from_xdd = common::sample_xdd();
    // The checked-in project enables callbacks on manufacturer objects, as
    // the original CanFestival .od did; the XDD has no such setting.
    for (i, o) in from_xdd.objects.iter_mut() {
        o.callback = *i >= 0x2000;
    }
    assert_eq!(from_xdd, from_toml);
}
