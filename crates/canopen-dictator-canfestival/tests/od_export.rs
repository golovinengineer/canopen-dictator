// SPDX-License-Identifier: LGPL-2.1-or-later
// Part of the Rust port of CanFestival objdictgen; see lib.rs for copyright.

mod common;

use canopen_dictator_canfestival::od::{self, Py};

/// Order-insensitive form of a pickled value: dict entries and object
/// attributes sorted, so two `.od` files can be compared by content.
fn normalize(v: &Py) -> Py {
    match v {
        Py::List(items) => Py::List(items.iter().map(normalize).collect()),
        Py::Dict(items) => {
            let mut items: Vec<_> = items
                .iter()
                .map(|(k, v)| (normalize(k), normalize(v)))
                .collect();
            items.sort_by_key(|(k, _)| format!("{k:?}"));
            Py::Dict(items)
        }
        Py::Object {
            module,
            class,
            attrs,
        } => {
            let mut attrs: Vec<_> = attrs
                .iter()
                .map(|(n, v)| (n.clone(), normalize(v)))
                .collect();
            attrs.sort_by(|a, b| a.0.cmp(&b.0));
            Py::Object {
                module: module.clone(),
                class: class.clone(),
                attrs,
            }
        }
        other => other.clone(),
    }
}

fn reference() -> Py {
    normalize(&od::parse_pickle(&common::read_asset("example.od")).unwrap())
}

/// Report the first differing attribute / dictionary key, not a 7000-line dump.
fn assert_same_node(ours: &Py, theirs: &Py) {
    let (Py::Object { attrs: a, .. }, Py::Object { attrs: b, .. }) = (ours, theirs) else {
        panic!("not objects");
    };
    let names = |x: &Vec<(String, Py)>| x.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
    assert_eq!(names(a), names(b), "attribute names");
    for ((name, x), (_, y)) in a.iter().zip(b) {
        if x == y {
            continue;
        }
        if let (Py::Dict(dx), Py::Dict(dy)) = (x, y) {
            for (k, vx) in dx {
                let vy = dy.iter().find(|(ky, _)| ky == k).map(|(_, v)| v);
                assert_eq!(Some(vx), vy, "{name}[{k:?}]");
            }
            for (k, _) in dy {
                assert!(
                    dx.iter().any(|(kx, _)| kx == k),
                    "{name}[{k:?}] missing in ours"
                );
            }
        }
        assert_eq!(x, y, "{name}");
    }
}

/// `.od` -> project -> `.od` keeps everything CanFestival stored.
#[test]
fn od_round_trip_matches_the_original_file() {
    let imported = od::load(&common::asset("example.od")).unwrap().od;
    let text = od::generate(&imported).unwrap();
    assert_same_node(&normalize(&od::parse_pickle(&text).unwrap()), &reference());
}

/// project -> `.od` gives what the old chain (XDD -> EDS -> objdictedit
/// import -> CanFestival settings -> save) stored in `example.od`.
#[test]
fn project_exports_the_reference_od() {
    let mut project = common::canfestival_project();
    // objdictedit's EDS import takes neither the device description nor
    // per-object descriptions over.
    project.device.description.clear();
    for o in project.objects.values_mut() {
        o.description.clear();
        o.subs.iter_mut().for_each(|s| s.description.clear());
    }
    let text = od::generate(&project).unwrap();
    assert_same_node(&normalize(&od::parse_pickle(&text).unwrap()), &reference());
}

/// Same project, same bytes; and the export reads back as the same project.
#[test]
fn export_is_stable_and_reimports() {
    let imported = od::load(&common::asset("example.od")).unwrap().od;
    let text = od::generate(&imported).unwrap();
    assert_eq!(text, od::generate(&imported).unwrap());
    let again = od::parse(&text).unwrap();
    assert!(again.warnings.is_empty(), "{:?}", again.warnings);
    assert_eq!(again.od.objects, imported.objects);
    assert_eq!(again.od.canfestival, imported.canfestival);
    assert_eq!(od::generate(&again.od).unwrap(), text);
}
