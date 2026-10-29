//! `.mdo` metadata objects (XML): configuration subsystems and metadata objects (D7, D8).
//! A subsystem becomes an L1 card; every other object is described lazily by an L2 card.

use crate::analysis::generator::sanitize_id;
use crate::analysis::lang::adapter_for_ext;
use crate::analysis::scope::ScopeFilter;
use crate::analysis::types::DiscoveredModule;
use crate::config::ScopeConfig;
use crate::storage::SubsystemFile;
use anyhow::{bail, Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// A parsed `.mdo` file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MdoObject {
    /// Root element name: `Subsystem`, `Catalog`, `Document`, ...
    pub kind: String,
    pub name: String,
    pub synonym: Option<String>,
    pub attributes: Vec<String>,
    /// `(section name, column names)`.
    pub tabular_sections: Vec<(String, Vec<String>)>,
    /// `Kind.Name` references listed in a subsystem's `<content>`.
    pub content_refs: Vec<String>,
    pub child_subsystems: Vec<String>,
    /// Exported methods of the object's modules (filled from sibling `.bsl` files).
    pub exported_methods: Vec<String>,
}

impl MdoObject {
    /// Stable id: `<kind>_<name>`, lower-case (`catalog_items`).
    pub fn id(&self) -> String {
        object_slug(&self.kind, &self.name)
    }
}

/// `<kind>_<name>` as an id. Names with non-ASCII letters would collapse to the same slug, so
/// they get a short digest of the name instead.
fn object_slug(kind: &str, name: &str) -> String {
    if name.is_ascii() {
        return sanitize_id(&format!("{kind}_{name}"));
    }
    let digest = Sha256::digest(name.as_bytes());
    let short: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
    format!("{}_{short}", sanitize_id(kind))
}

fn subsystem_module_id(name: &str) -> String {
    object_slug("subsystem", name)
}

/// Parses one `.mdo` XML document. Namespaces are ignored; elements are matched by local name.
pub fn parse_mdo_xml(xml: &str) -> Result<MdoObject> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<String> = Vec::new();
    let mut obj = MdoObject {
        kind: String::new(),
        name: String::new(),
        synonym: None,
        attributes: Vec::new(),
        tabular_sections: Vec::new(),
        content_refs: Vec::new(),
        child_subsystems: Vec::new(),
        exported_methods: Vec::new(),
    };
    loop {
        match reader.read_event().context("invalid .mdo XML")? {
            Event::Start(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if stack.is_empty() {
                    obj.kind = name.clone();
                } else if stack.len() == 1 && name == "tabularSections" {
                    obj.tabular_sections.push((String::new(), Vec::new()));
                }
                stack.push(name);
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Text(t) => {
                let text = t
                    .unescape()
                    .context("invalid text in .mdo")?
                    .trim()
                    .to_string();
                if text.is_empty() {
                    continue;
                }
                let path: Vec<&str> = stack.iter().map(String::as_str).collect();
                match path.as_slice() {
                    [_, "name"] => obj.name = text,
                    [_, "synonym", "value"] if obj.synonym.is_none() => obj.synonym = Some(text),
                    [_, "attributes", "name"] => obj.attributes.push(text),
                    [_, "tabularSections", "name"] => {
                        if let Some(section) = obj.tabular_sections.last_mut() {
                            section.0 = text;
                        }
                    }
                    [_, "tabularSections", "attributes", "name"] => {
                        if let Some(section) = obj.tabular_sections.last_mut() {
                            section.1.push(text);
                        }
                    }
                    [_, "content"] => obj.content_refs.push(text),
                    [_, "subsystems"] | [_, "subsystems", "name"] => {
                        obj.child_subsystems.push(text)
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if obj.kind.is_empty() || obj.name.is_empty() {
        bail!(".mdo has no root element or <name>");
    }
    Ok(obj)
}

/// Markdown L2 card of a metadata object.
pub fn render_mdo_l2_card(obj: &MdoObject) -> String {
    let mut md = format!("# Объект метаданных: {}.{} (L2)\n\n", obj.kind, obj.name);
    match &obj.synonym {
        Some(s) => md.push_str(&format!("{s}\n\n")),
        None => md.push_str(&format!("Объект `{}.{}`.\n\n", obj.kind, obj.name)),
    }
    md.push_str(&format!("> **Идентификатор:** `{}`\n\n", obj.id()));
    let section = |md: &mut String, title: &str, items: &[String]| {
        if !items.is_empty() {
            md.push_str(&format!("## {title}\n"));
            for i in items {
                md.push_str(&format!("- `{i}`\n"));
            }
            md.push('\n');
        }
    };
    section(&mut md, "Реквизиты", &obj.attributes);
    if !obj.tabular_sections.is_empty() {
        md.push_str("## Табличные части\n");
        for (name, cols) in &obj.tabular_sections {
            md.push_str(&format!("- `{name}`: {}\n", cols.join(", ")));
        }
        md.push('\n');
    }
    section(&mut md, "Состав подсистемы", &obj.content_refs);
    section(&mut md, "Дочерние подсистемы", &obj.child_subsystems);
    section(&mut md, "Экспортные методы", &obj.exported_methods);
    md
}

fn is_module(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".bsl") || lower.ends_with(".os")
}

fn parent_dir(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(d, _)| d)
}

fn exported_methods_in(root: &Path, dir: &str, files: &[String]) -> Vec<String> {
    let Some(adapter) = adapter_for_ext("bsl") else {
        return Vec::new();
    };
    let mut methods = Vec::new();
    for f in files
        .iter()
        .filter(|f| is_module(f) && parent_dir(f) == dir)
    {
        if let Ok(src) = fs::read_to_string(root.join(f)) {
            for m in adapter.extract_symbols(&src) {
                if !methods.contains(&m) {
                    methods.push(m);
                }
            }
        }
    }
    methods
}

/// Every parsable `.mdo` of `files` (relative, already scoped) with its exported methods.
fn load_objects(root: &Path, files: &[String]) -> Vec<(String, MdoObject)> {
    let mut out = Vec::new();
    for rel in files
        .iter()
        .filter(|f| f.to_ascii_lowercase().ends_with(".mdo"))
    {
        let Ok(xml) = fs::read_to_string(root.join(rel)) else {
            continue;
        };
        if let Ok(mut obj) = parse_mdo_xml(&xml) {
            obj.exported_methods = exported_methods_in(root, parent_dir(rel), files);
            out.push((rel.clone(), obj));
        }
    }
    out
}

fn subsystem_file(root: &Path, rel: &str, entrypoint: bool) -> SubsystemFile {
    let lines = fs::read_to_string(root.join(rel)).map_or(0, |t| t.lines().count());
    SubsystemFile {
        path: rel.to_string(),
        is_entrypoint: entrypoint,
        lines,
    }
}

/// L1 modules of a metadata project: one per configuration subsystem (with the files of the
/// objects in its `<content>`) and one per kind of objects that belong to no subsystem.
/// Returns the modules and the set of files they cover.
pub(crate) fn discover_mdo_subsystems(
    root: &Path,
    files: &[String],
) -> (Vec<DiscoveredModule>, BTreeSet<String>) {
    let objects = load_objects(root, files);
    let mut covered: BTreeSet<String> = BTreeSet::new();
    let mut modules = Vec::new();
    if objects.is_empty() {
        return (modules, covered);
    }
    // `Kind.Name` -> directory of the object
    let dir_of: BTreeMap<String, &str> = objects
        .iter()
        .map(|(rel, o)| (format!("{}.{}", o.kind, o.name), parent_dir(rel)))
        .collect();
    let mut owned_dirs: BTreeSet<&str> = BTreeSet::new();

    let files_in = |dir: &str| -> Vec<String> {
        files
            .iter()
            .filter(|f| {
                parent_dir(f) == dir && (is_module(f) || f.to_ascii_lowercase().ends_with(".mdo"))
            })
            .cloned()
            .collect()
    };
    let module_of = |id: String,
                     name: String,
                     dir_path: String,
                     mut rels: Vec<String>,
                     entry: Option<&str>|
     -> DiscoveredModule {
        rels.sort();
        rels.dedup();
        rels.sort_by_key(|r| (Some(r.as_str()) != entry, r.clone()));
        rels.truncate(15);
        let symbols: Vec<String> = rels
            .iter()
            .filter(|r| is_module(r))
            .filter_map(|r| fs::read_to_string(root.join(r)).ok())
            .flat_map(|src| {
                adapter_for_ext("bsl")
                    .map(|a| a.extract_symbols(&src))
                    .unwrap_or_default()
            })
            .collect();
        DiscoveredModule {
            id,
            parent: None,
            dir_path,
            name,
            entrypoints: entry.map(|e| vec![e.to_string()]).unwrap_or_default(),
            files: rels
                .iter()
                .map(|r| subsystem_file(root, r, Some(r.as_str()) == entry))
                .collect(),
            symbols,
            test_cmd: None,
        }
    };

    for (rel, obj) in objects.iter().filter(|(_, o)| o.kind == "Subsystem") {
        let mut rels = vec![rel.clone()];
        owned_dirs.insert(parent_dir(rel));
        for reference in &obj.content_refs {
            if let Some(dir) = dir_of.get(reference) {
                owned_dirs.insert(dir);
                rels.extend(files_in(dir));
            }
        }
        covered.extend(rels.iter().cloned());
        modules.push(module_of(
            subsystem_module_id(&obj.name),
            obj.name.clone(),
            parent_dir(rel).to_string(),
            rels,
            Some(rel.as_str()),
        ));
    }

    let mut by_kind: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (rel, obj) in objects.iter().filter(|(_, o)| o.kind != "Subsystem") {
        if owned_dirs.contains(parent_dir(rel)) {
            continue;
        }
        by_kind
            .entry(obj.kind.clone())
            .or_default()
            .extend(files_in(parent_dir(rel)));
    }
    for (kind, rels) in by_kind {
        covered.extend(rels.iter().cloned());
        let dir = rels
            .first()
            .map(|r| parent_dir(parent_dir(r)).to_string())
            .unwrap_or_default();
        modules.push(module_of(
            sanitize_id(&format!("metadata_{kind}")),
            format!("{kind} objects"),
            dir,
            rels,
            None,
        ));
    }
    (modules, covered)
}

/// L2 card of the metadata object with `id` (`catalog_items`), generated on demand from the
/// project tree; `None` when no such object exists.
pub fn find_object_card(root: &Path, scope: &ScopeConfig, id: &str) -> Result<Option<String>> {
    let filter = ScopeFilter::build(root, scope)?;
    let wanted = id.trim().to_ascii_lowercase();
    Ok(load_objects(root, filter.files())
        .into_iter()
        .find(|(_, o)| o.id() == wanted && o.kind != "Subsystem")
        .map(|(_, o)| render_mdo_l2_card(&o)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUBSYSTEM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<mdclass:Subsystem xmlns:mdclass="urn:example:mdclass" uuid="a1">
  <name>Billing</name>
  <synonym><key>ru</key><value>Расчёты</value></synonym>
  <content>Catalog.Items</content>
  <content>Document.Invoice</content>
  <subsystems>Invoicing</subsystems>
</mdclass:Subsystem>"#;

    const CATALOG: &str = "<?xml version=\"1.0\"?>\r\n<mdclass:Catalog xmlns:mdclass=\"urn:example:mdclass\">\r\n  <name>Items</name>\r\n  <synonym><key>en</key><value>Items &amp; goods</value></synonym>\r\n  <attributes><name>Code</name></attributes>\r\n  <attributes><name>Price</name></attributes>\r\n  <tabularSections><name>Prices</name><attributes><name>Period</name></attributes><attributes><name>Value</name></attributes></tabularSections>\r\n</mdclass:Catalog>";

    #[test]
    fn parse_mdo_xml_extracts_subsystem_and_metadata_object_fields() {
        let sub = parse_mdo_xml(SUBSYSTEM).unwrap();
        assert_eq!(sub.kind, "Subsystem");
        assert_eq!(sub.name, "Billing");
        assert_eq!(sub.synonym.as_deref(), Some("Расчёты"));
        assert_eq!(sub.content_refs, ["Catalog.Items", "Document.Invoice"]);
        assert_eq!(sub.child_subsystems, ["Invoicing"]);

        let cat = parse_mdo_xml(CATALOG).unwrap();
        assert_eq!((cat.kind.as_str(), cat.name.as_str()), ("Catalog", "Items"));
        assert_eq!(cat.synonym.as_deref(), Some("Items & goods"));
        assert_eq!(cat.attributes, ["Code", "Price"]);
        assert_eq!(
            cat.tabular_sections,
            [(
                "Prices".to_string(),
                vec!["Period".to_string(), "Value".to_string()]
            )]
        );
        assert_eq!(cat.id(), "catalog_items");
        assert!(parse_mdo_xml("<a><b></a>").is_err());
        assert!(parse_mdo_xml("").is_err());
    }

    #[test]
    fn non_ascii_object_names_get_distinct_stable_ids() {
        let a = parse_mdo_xml("<m:Subsystem xmlns:m=\"urn:x\"><name>Продажи</name></m:Subsystem>")
            .unwrap();
        let b = parse_mdo_xml("<m:Subsystem xmlns:m=\"urn:x\"><name>Закупки</name></m:Subsystem>")
            .unwrap();
        assert_ne!(a.id(), b.id());
        assert_eq!(a.id(), a.id());
        assert!(a.id().starts_with("subsystem_"), "{}", a.id());
        assert_ne!(
            subsystem_module_id("Продажи"),
            subsystem_module_id("Закупки")
        );
        assert_eq!(subsystem_module_id("Billing"), "subsystem_billing");
    }
}
