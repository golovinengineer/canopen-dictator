// SPDX-License-Identifier: MIT OR Apache-2.0

mod common;

use dictator_core::editor::{Editor, ListRow, parse_hex_index, value_ok};
use dictator_core::model::*;

fn fresh() -> Editor {
    Editor::new(ObjectDictionary::new_default(), None, false)
}

#[test]
fn new_session_selects_first_object_and_is_clean() {
    let e = fresh();
    assert_eq!(e.selected(), Some(0x1000));
    assert_eq!(e.selected_sub(), Some(0));
    assert!(!e.dirty());
    assert_eq!(e.file_label(), "Untitled");
    assert_eq!(e.project_stem(), "ObjDict");
    // An import has never been saved.
    assert!(Editor::new(ObjectDictionary::new_default(), None, true).dirty());
}

#[test]
fn edit_then_revert_is_clean() {
    let mut e = fresh();
    e.edit_object("name", "Something else").unwrap();
    assert!(e.dirty());
    e.edit_object("name", "Device type").unwrap();
    assert!(!e.dirty());
}

#[test]
fn var_name_follows_object_name() {
    let mut e = fresh();
    e.edit_object("name", "Kind of device").unwrap();
    assert_eq!(e.selected_sub_object().unwrap().name, "Kind of device");
    // The sub name of a VAR can't be set apart from the object's.
    e.edit_sub("name", "Other").unwrap();
    assert_eq!(e.selected_sub_object().unwrap().name, "Kind of device");
}

#[test]
fn hex_index_parsing() {
    assert_eq!(parse_hex_index("0x2000").unwrap(), 0x2000);
    assert_eq!(parse_hex_index(" 6a00 ").unwrap(), 0x6A00);
    assert!(parse_hex_index("0FFF").is_err());
    assert!(parse_hex_index("zz").is_err());
}

#[test]
fn add_renumber_delete_object() {
    let mut e = fresh();
    e.set_filter("identity");
    assert_eq!(e.add_object("2000", "  ", "array").unwrap(), 0x2000);
    assert_eq!(e.filter(), "", "adding clears the filter so the object shows");
    assert_eq!(e.selected(), Some(0x2000));
    assert_eq!(e.od.objects[&0x2000].name, "New object");
    assert!(e.add_object("2000", "Again", "var").is_err());
    assert!(e.add_object("2001", "Bad kind", "LIST").is_err());

    assert!(e.edit_object("index", "1000").is_err(), "index in use");
    e.edit_object("index", "2100").unwrap();
    assert_eq!(e.selected(), Some(0x2100));
    assert!(!e.od.objects.contains_key(&0x2000));

    assert_eq!(e.delete_object().unwrap(), 0x2100);
    // Nothing after 0x2100: the one before is selected.
    assert_eq!(e.selected(), Some(0x1018));
    e.select_object(Some(0x1000));
    e.delete_object().unwrap();
    assert_eq!(e.selected(), Some(0x1001));
}

#[test]
fn index_and_setting_checks() {
    let e = fresh();
    assert!(e.index_ok("2000", None));
    assert!(!e.index_ok("1000", None));
    assert!(e.index_ok("1000", Some(0x1000)), "an object's own index is fine");
    assert!(!e.index_ok("0x0100", None));
    assert!(e.setting_ok("node_id", "127"));
    assert!(!e.setting_ok("node_id", "128"));
    assert!(!e.setting_ok("node_type", "boss"));
    assert!(e.setting_ok("baud_rates", "125, 250"));
    assert!(!e.setting_ok("granularity", "x"));
    assert!(value_ok("$NODEID+0x180", DataType::U32));
    assert!(!value_ok("300", DataType::U8));
}

#[test]
fn settings_round_trip() {
    let mut e = fresh();
    e.edit_setting("baud_rates", "125,  250,").unwrap();
    assert_eq!(e.od.device.baud_rates, vec![125, 250]);
    e.edit_setting("node_type", "master").unwrap();
    let fields = e.canfestival_fields();
    assert_eq!(fields.iter().find(|f| f.key == "node_type").unwrap().value, "master");
    let rates = e.device_fields().into_iter().find(|f| f.key == "baud_rates").unwrap();
    assert_eq!(rates.value, "125, 250");
    assert!(e.edit_setting("nope", "1").is_err());
}

#[test]
fn change_kind_keeps_sub0_in_sync() {
    let mut e = fresh();
    e.select_object(Some(0x1000));
    e.edit_object("kind", "array").unwrap();
    let o = &e.od.objects[&0x1000];
    assert_eq!(o.subs.len(), 2);
    assert_eq!(o.subs[0].default, "0x01");
    assert_eq!(o.subs[1].sub, 1);
    assert_eq!(o.subs[1].default, "0x00000000", "the VAR's value becomes element 1");

    e.edit_object("kind", "var").unwrap();
    let o = &e.od.objects[&0x1000];
    assert_eq!(o.subs.len(), 1);
    assert_eq!(o.subs[0].sub, 0);
    assert_eq!(o.subs[0].name, "Device type");
}

#[test]
fn add_and_delete_subs() {
    let mut e = fresh();
    e.select_object(Some(0x1000));
    assert!(e.add_sub().is_err(), "VAR has no subindexes");

    e.add_object("2000", "Table", "array").unwrap();
    e.add_sub().unwrap();
    e.edit_sub("type", "UNSIGNED16").unwrap();
    e.edit_sub("default", "7").unwrap();
    e.add_sub().unwrap();
    let o = &e.od.objects[&0x2000];
    assert_eq!(o.subs.len(), 3);
    assert_eq!(o.subs[0].default, "0x02");
    assert_eq!(o.subs[2].data_type, DataType::U16, "array elements copy the previous one");
    assert_eq!(o.subs[2].default, "7");
    assert_eq!(e.selected_sub(), Some(2));

    e.select_sub(Some(0));
    assert!(e.delete_sub().is_err(), "sub 0 stays");
    e.select_sub(Some(2));
    e.delete_sub().unwrap();
    assert_eq!(e.selected_sub(), Some(1));
    assert_eq!(e.od.objects[&0x2000].subs[0].default, "0x01");

    assert!(e.edit_sub("type", "BOGUS").is_err());
    assert!(e.edit_sub("colour", "red").is_err());
}

#[test]
fn filter_groups_matching_objects() {
    let mut e = Editor::new(common::sample_project(), None, false);
    let all = e.visible_objects();
    assert!(matches!(all[0], ListRow::Header(t) if t.starts_with("Communication")));
    e.set_filter("0x1018");
    assert_eq!(e.visible_objects(), vec![ListRow::Header(dictator_core::editor::group_title(0x1018)), ListRow::Object(0x1018)]);
    e.set_filter("no object is called this");
    assert!(e.visible_objects().is_empty());
}

#[test]
fn save_marks_clean() {
    let dir = std::env::temp_dir().join(format!("dictator-editor-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("t.objdict.toml");
    let mut e = Editor::new(ObjectDictionary::new_default(), None, true);
    e.save_to(path.clone()).unwrap();
    assert!(!e.dirty());
    assert_eq!(e.file_label(), "t.objdict.toml");
    assert_eq!(e.project_stem(), "t");
    std::fs::remove_dir_all(&dir).unwrap();
}
