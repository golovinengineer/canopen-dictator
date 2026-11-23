// SPDX-License-Identifier: LGPL-2.1-or-later
// Part of the Rust port of CanFestival objdictgen; see lib.rs for copyright.

//! Reading and writing the subset of `gnosis.xml.pickle` that `.od` files use.

use std::collections::HashMap;
use std::fmt::Write as _;

use anyhow::{Result, anyhow, bail};
use roxmltree::{Document, Node, ParsingOptions};

// ------------------------------------------------------------ gnosis pickle

/// A Python value as written by `gnosis.xml.pickle`.
#[derive(Debug, Clone, PartialEq)]
pub enum Py {
    None,
    Bool(bool),
    Int(i128),
    /// Kept as written (`1.5`, `1e-05`).
    Float(String),
    Str(String),
    List(Vec<Py>),
    Dict(Vec<(Py, Py)>),
    Object { module: String, class: String, attrs: Vec<(String, Py)> },
}

impl Py {
    /// Dictionary lookup (dicts here are small; keys are ints or strings).
    pub(crate) fn get(&self, key: &Py) -> Option<&Py> {
        match self {
            Py::Dict(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn get_str(&self, key: &str) -> Option<&Py> {
        self.get(&Py::Str(key.to_string()))
    }

    pub(crate) fn attr(&self, name: &str) -> Option<&Py> {
        match self {
            Py::Object { attrs, .. } => attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn as_int(&self) -> Option<i128> {
        match self {
            Py::Int(i) => Some(*i),
            Py::Bool(b) => Some(*b as i128),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Py::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Python truthiness, for flags stored as bools or ints.
    pub(crate) fn truthy(&self) -> bool {
        match self {
            Py::None => false,
            Py::Bool(b) => *b,
            Py::Int(i) => *i != 0,
            Py::Float(f) => f.parse::<f64>().is_ok_and(|f| f != 0.0),
            Py::Str(s) => !s.is_empty(),
            Py::List(l) => !l.is_empty(),
            Py::Dict(d) => !d.is_empty(),
            Py::Object { .. } => true,
        }
    }
}

struct Pickle<'a, 'input> {
    /// Elements by `id`, for `refid` back-references to shared objects.
    ids: HashMap<&'a str, Node<'a, 'input>>,
}

impl<'a, 'input> Pickle<'a, 'input> {
    fn value(&self, n: Node<'a, 'input>, depth: u32) -> Result<Py> {
        if depth > 64 {
            bail!("pickle nested too deeply (reference cycle?)");
        }
        if let Some(r) = n.attribute("refid") {
            let target = *self.ids.get(r).ok_or_else(|| anyhow!("dangling refid {r}"))?;
            return self.value(target, depth + 1);
        }
        let ty = n
            .attribute("type")
            .or_else(|| (n.tag_name().name() == "PyObject").then_some("PyObject"))
            .ok_or_else(|| anyhow!("<{}> without a type", n.tag_name().name()))?;
        let raw = n.attribute("value").unwrap_or("");
        let kids = |tag: &'static str| n.children().filter(move |c| c.tag_name().name() == tag);
        Ok(match ty {
            "None" => Py::None,
            "True" => Py::Bool(true),
            "False" => Py::Bool(false),
            "numeric" => {
                let t = raw.trim().trim_end_matches(['L', 'l']);
                match t.parse::<i128>() {
                    Ok(i) => Py::Int(i),
                    Err(_) if t.parse::<f64>().is_ok() => Py::Float(t.to_string()),
                    Err(_) => bail!("unsupported number {raw:?}"),
                }
            }
            "string" | "unicode" => Py::Str(unsafe_string(raw)),
            "list" | "tuple" => Py::List(
                kids("item")
                    .map(|c| self.value(c, depth + 1))
                    .collect::<Result<_>>()?,
            ),
            "dict" => {
                let mut items = Vec::new();
                for e in kids("entry") {
                    let part = |tag: &str| {
                        e.children()
                            .find(|c| c.tag_name().name() == tag)
                            .ok_or_else(|| anyhow!("dict entry without <{tag}>"))
                    };
                    items.push((self.value(part("key")?, depth + 1)?, self.value(part("val")?, depth + 1)?));
                }
                Py::Dict(items)
            }
            "PyObject" => Py::Object {
                module: n.attribute("module").unwrap_or("").to_string(),
                class: n.attribute("class").unwrap_or("").to_string(),
                attrs: kids("attr")
                    .map(|c| {
                        let name = c.attribute("name").unwrap_or("").to_string();
                        Ok((name, self.value(c, depth + 1)?))
                    })
                    .collect::<Result<_>>()?,
            },
            other => bail!("unsupported pickled type {other:?}"),
        })
    }
}

/// Parse a `gnosis.xml.pickle` document into its root object.
pub fn parse_pickle(text: &str) -> Result<Py> {
    let opts = ParsingOptions { allow_dtd: true, ..ParsingOptions::default() };
    let doc = Document::parse_with_options(text, opts)?;
    let root = doc.root_element();
    if root.tag_name().name() != "PyObject" {
        bail!("not a CanFestival .od file (root element <{}>)", root.tag_name().name());
    }
    let ids = doc
        .descendants()
        .filter_map(|n| n.attribute("id").map(|id| (id, n)))
        .collect();
    Pickle { ids }.value(root, 0)
}
// ---------------------------------------------------------------- writing

/// XML-escape markup characters (names, module and class attributes).
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `gnosis.xml.pickle.util.safe_string`: XML entities, then Python 2
/// `repr()` escapes on the bytes (the reader `exec`s the text as a Python
/// string literal, so a raw line break or backslash would break it).
fn safe_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'&' => out.push_str("&amp;"),
            b'<' => out.push_str("&lt;"),
            b'>' => out.push_str("&gt;"),
            b'"' => out.push_str("&quot;"),
            b'\'' => out.push_str("&apos;"),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7E => out.push(b as char),
            _ => {
                let _ = write!(out, "\\x{b:02x}");
            }
        }
    }
    out
}

/// `gnosis.xml.pickle.util.unsafe_string`: undo the Python escapes of an
/// attribute value the XML parser already decoded. Bytes that are not
/// UTF-8 are taken as Latin-1.
fn unsafe_string(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' || i + 1 == b.len() {
            out.push(b[i]);
            i += 1;
            continue;
        }
        let c = b[i + 1];
        i += 2;
        match c {
            b'\\' | b'\'' | b'"' => out.push(c),
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            b'a' => out.push(0x07),
            b'b' => out.push(0x08),
            b'f' => out.push(0x0C),
            b'v' => out.push(0x0B),
            b'\n' => {} // line continuation
            b'x' if i + 2 <= b.len()
                && let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i..i + 2]).unwrap_or(""), 16) =>
            {
                out.push(v);
                i += 2;
            }
            b'0'..=b'7' => {
                let mut v = (c - b'0') as u32;
                let mut n = 1;
                while n < 3 && i < b.len() && (b'0'..=b'7').contains(&b[i]) {
                    v = v * 8 + (b[i] - b'0') as u32;
                    i += 1;
                    n += 1;
                }
                out.push(v as u8);
            }
            // Python 2 keeps unknown escapes as written.
            other => out.extend_from_slice(&[b'\\', other]),
        }
    }
    String::from_utf8(out).unwrap_or_else(|e| e.into_bytes().iter().map(|&b| b as char).collect())
}

struct Writer {
    out: String,
    next_id: u64,
}

impl Writer {
    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// `<tag name="..." type="..." .../>` or a container with children.
    fn value(&mut self, indent: usize, tag: &str, name: Option<&str>, v: &Py) {
        let pad = " ".repeat(indent);
        let name = name.map(|n| format!(" name=\"{}\"", escape(n))).unwrap_or_default();
        let scalar = |ty: &str, value: &str| format!("{pad}<{tag}{name} type=\"{ty}\" value=\"{value}\" />\n");
        match v {
            Py::None => {
                let _ = writeln!(self.out, "{pad}<{tag}{name} type=\"None\" />");
            }
            Py::Bool(b) => self.out += &scalar(if *b { "True" } else { "False" }, ""),
            Py::Int(i) => self.out += &scalar("numeric", &i.to_string()),
            Py::Float(f) => self.out += &scalar("numeric", &escape(f)),
            Py::Str(t) => self.out += &scalar("string", &safe_string(t)),
            Py::List(items) => {
                let id = self.id();
                let _ = writeln!(self.out, "{pad}<{tag}{name} type=\"list\" id=\"{id}\" >");
                for item in items {
                    self.value(indent + 2, "item", None, item);
                }
                let _ = writeln!(self.out, "{pad}</{tag}>");
            }
            Py::Dict(items) => {
                let id = self.id();
                let _ = writeln!(self.out, "{pad}<{tag}{name} type=\"dict\" id=\"{id}\" >");
                for (k, v) in items {
                    let _ = writeln!(self.out, "{pad}  <entry>");
                    self.value(indent + 4, "key", None, k);
                    self.value(indent + 4, "val", None, v);
                    let _ = writeln!(self.out, "{pad}  </entry>");
                }
                let _ = writeln!(self.out, "{pad}</{tag}>");
            }
            Py::Object { module, class, attrs } => {
                let id = self.id();
                let _ = writeln!(
                    self.out,
                    "{pad}<{tag}{name} type=\"PyObject\" module=\"{}\" class=\"{}\" id=\"{id}\" >",
                    escape(module),
                    escape(class)
                );
                for (n, v) in attrs {
                    self.value(indent + 2, "attr", Some(n), v);
                }
                let _ = writeln!(self.out, "{pad}</{tag}>");
            }
        }
    }
}

/// Write `root` (a [`Py::Object`]) as a `gnosis.xml.pickle` document, laid
/// out like the files objdictedit saves. Ids are sequential, so the output
/// depends only on the value.
pub fn write_pickle(root: &Py) -> String {
    let mut w = Writer { out: String::new(), next_id: 0 };
    w.out += "<?xml version=\"1.0\"?>\n<!DOCTYPE PyObject SYSTEM \"PyObjects.dtd\">\n";
    match root {
        Py::Object { module, class, attrs } => {
            let id = w.id();
            let _ = writeln!(
                w.out,
                "<PyObject module=\"{}\" class=\"{}\" id=\"{id}\">",
                escape(module),
                escape(class)
            );
            for (n, v) in attrs {
                w.value(0, "attr", Some(n), v);
            }
            w.out += "</PyObject>\n";
        }
        other => w.value(0, "item", None, other),
    }
    w.out
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = r#"<?xml version="1.0"?>
<!DOCTYPE PyObject SYSTEM "PyObjects.dtd">
<PyObject module="node" class="Node" id="1">"#;

    fn doc(body: &str) -> String {
        format!("{HEAD}{body}</PyObject>")
    }

    #[test]
    fn written_pickle_reads_back() {
        let root = Py::Object {
            module: "node".into(),
            class: "Node".into(),
            attrs: vec![
                ("S".into(), Py::Str("a<b> & \"c\" 'd'\nline\t\\ 25 \u{b0}C".into())),
                (
                    "D".into(),
                    Py::Dict(vec![
                        (Py::Int(4096), Py::List(vec![Py::Bool(false), Py::Float("1.5".into())])),
                        (Py::Str("k".into()), Py::Dict(Vec::new())),
                    ]),
                ),
                ("N".into(), Py::None),
            ],
        };
        let text = write_pickle(&root);
        assert_eq!(parse_pickle(&text).unwrap(), root);
        assert!(text.contains("  <entry>\n    <key type=\"numeric\" value=\"4096\" />"), "{text}");
        // What Python 2 gnosis writes for the same string.
        assert!(
            text.contains(r#"value="a&lt;b&gt; &amp; &quot;c&quot; &apos;d&apos;\nline\t\\ 25 \xc2\xb0C""#),
            "{text}"
        );
    }

    #[test]
    fn python_escapes_are_read() {
        assert_eq!(unsafe_string(r"a\nb\\c\x41\047\q"), "a\nb\\cA'\\q");
        assert_eq!(unsafe_string(r"25 \xc2\xb0C"), "25 \u{b0}C");
        assert_eq!(unsafe_string(r"\xff"), "\u{ff}");
    }

    #[test]
    fn pickle_values_and_back_references() {
        let py = parse_pickle(&doc(r#"
<attr name="L" type="list" id="2">
  <item type="numeric" value="4294967296L" />
  <item type="numeric" value="1.5" />
  <item type="True" value="" />
  <item type="None" />
  <item type="string" value="a &quot;b&quot;" />
</attr>
<attr name="R" type="list" refid="2" />"#))
        .unwrap();
        let expected = Py::List(vec![
            Py::Int(4294967296),
            Py::Float("1.5".into()),
            Py::Bool(true),
            Py::None,
            Py::Str("a \"b\"".into()),
        ]);
        assert_eq!(py.attr("L"), Some(&expected));
        assert_eq!(py.attr("R"), Some(&expected));
    }

}
