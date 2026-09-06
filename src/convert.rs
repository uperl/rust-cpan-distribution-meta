//! Normalisation of raw metadata (any spec version) into the version 2
//! structure, closely following `CPAN::Meta::Converter`.
//!
//! The input is a mutable JSON object (YAML is converted to the same value
//! model before this runs). Everything here is lenient: malformed or
//! unexpected shapes are coerced or dropped rather than rejected, so that the
//! result always deserialises cleanly into [`crate::Meta`].

use serde_json::{Map, Value};

use crate::spec::SpecVersion;

/// Legacy top-level prerequisite keys and the `(phase, relationship)` they map
/// to in the version 2 `prereqs` structure.
const LEGACY_PREREQS: &[(&str, &str, &str)] = &[
    ("configure_requires", "configure", "requires"),
    ("build_requires", "build", "requires"),
    ("test_requires", "test", "requires"),
    ("requires", "runtime", "requires"),
    ("recommends", "runtime", "recommends"),
    ("conflicts", "runtime", "conflicts"),
];

/// Normalise `obj` in place to the spec version 2 structure.
pub(crate) fn normalize(obj: &mut Map<String, Value>, _spec: &SpecVersion) {
    obj.remove("distribution_type");
    obj.remove("license_uri");

    normalize_meta_spec(obj);
    normalize_scalars(obj);
    normalize_author(obj);
    normalize_license(obj);
    normalize_keywords(obj);
    normalize_prereqs(obj);
    normalize_optional_features(obj);
    normalize_provides(obj);
    normalize_no_index(obj);
    normalize_resources(obj);
    normalize_dynamic_config(obj);
    normalize_release_status(obj);
}

fn normalize_meta_spec(obj: &mut Map<String, Value>) {
    if !obj.contains_key("meta-spec")
        && let Some(v) = obj.remove("meta_spec")
    {
        obj.insert("meta-spec".to_string(), v);
    }

    let url = obj
        .get("meta-spec")
        .and_then(Value::as_object)
        .and_then(|m| m.get("url"))
        .and_then(scalar_to_string);

    let mut m = Map::new();
    m.insert("version".to_string(), Value::String("2".to_string()));
    if let Some(url) = url {
        m.insert("url".to_string(), Value::String(url));
    }
    obj.insert("meta-spec".to_string(), Value::Object(m));
}

fn normalize_scalars(obj: &mut Map<String, Value>) {
    for key in ["name", "version", "abstract", "description", "generated_by"] {
        if let Some(v) = obj.get(key) {
            if let Some(s) = scalar_to_string(v) {
                obj.insert(key.to_string(), Value::String(s));
            } else {
                obj.remove(key);
            }
        }
    }

    if !matches!(obj.get("version"), Some(Value::String(_))) {
        obj.insert("version".to_string(), Value::String("0".to_string()));
    }
}

fn normalize_author(obj: &mut Map<String, Value>) {
    let value = obj.remove("author").or_else(|| obj.remove("authors"));
    obj.remove("authors");

    if let Some(value) = value {
        let list = to_string_list(&value);
        if !list.is_empty() {
            obj.insert(
                "author".to_string(),
                Value::Array(list.into_iter().map(Value::String).collect()),
            );
        }
    }
}

fn normalize_license(obj: &mut Map<String, Value>) {
    if let Some(value) = obj.remove("license") {
        let mapped: Vec<Value> = to_string_list(&value)
            .iter()
            .map(|t| Value::String(normalize_license_token(t)))
            .collect();
        if !mapped.is_empty() {
            obj.insert("license".to_string(), Value::Array(mapped));
        }
    }
}

/// Map a license token to a spec version 2 identifier. Known v2 tokens pass
/// through; known 1.x tokens are upgraded; anything else becomes `unknown`.
fn normalize_license_token(token: &str) -> String {
    let t = token.trim().to_lowercase();
    const V2: &[&str] = &[
        "agpl_3",
        "apache_1_1",
        "apache_2_0",
        "artistic_1",
        "artistic_2",
        "bsd",
        "freebsd",
        "gfdl_1_2",
        "gfdl_1_3",
        "gpl_1",
        "gpl_2",
        "gpl_3",
        "lgpl_2_1",
        "lgpl_3_0",
        "mit",
        "mozilla_1_0",
        "mozilla_1_1",
        "openssl",
        "perl_5",
        "qpl_1_0",
        "ssleay",
        "sun",
        "zlib",
        "open_source",
        "restricted",
        "unrestricted",
        "unknown",
    ];
    if V2.contains(&t.as_str()) {
        return t;
    }
    match t.as_str() {
        "perl" => "perl_5",
        "apache" => "apache_2_0",
        "artistic" => "artistic_1",
        "mozilla" => "mozilla_1_0",
        "gpl" | "lgpl" => "open_source",
        "restrictive" => "restricted",
        _ => "unknown",
    }
    .to_string()
}

fn normalize_keywords(obj: &mut Map<String, Value>) {
    if let Some(value) = obj.remove("keywords") {
        let list = to_string_list(&value);
        if !list.is_empty() {
            obj.insert(
                "keywords".to_string(),
                Value::Array(list.into_iter().map(Value::String).collect()),
            );
        }
    }
}

fn normalize_prereqs(obj: &mut Map<String, Value>) {
    let mut prereqs = obj
        .remove("prereqs")
        .and_then(value_into_object)
        .unwrap_or_default();

    for (key, phase, rel) in LEGACY_PREREQS {
        if let Some(value) = obj.remove(*key) {
            merge_deps(&mut prereqs, phase, rel, &value);
        }
    }

    sanitize_prereqs(&mut prereqs);

    if !prereqs.is_empty() {
        obj.insert("prereqs".to_string(), Value::Object(prereqs));
    }
}

fn normalize_optional_features(obj: &mut Map<String, Value>) {
    let Some(value) = obj.remove("optional_features") else {
        return;
    };
    let Some(features) = value.as_object() else {
        return;
    };

    let mut fixed = Map::new();
    for (name, fval) in features {
        let Some(fmap) = fval.as_object() else {
            continue;
        };

        let mut out = Map::new();
        if let Some(desc) = fmap.get("description").and_then(scalar_to_string) {
            out.insert("description".to_string(), Value::String(desc));
        }

        let mut prereqs = fmap
            .get("prereqs")
            .cloned()
            .and_then(value_into_object)
            .unwrap_or_default();
        for (key, phase, rel) in LEGACY_PREREQS {
            if let Some(v) = fmap.get(*key) {
                merge_deps(&mut prereqs, phase, rel, v);
            }
        }
        sanitize_prereqs(&mut prereqs);
        if !prereqs.is_empty() {
            out.insert("prereqs".to_string(), Value::Object(prereqs));
        }

        fixed.insert(name.clone(), Value::Object(out));
    }

    if !fixed.is_empty() {
        obj.insert("optional_features".to_string(), Value::Object(fixed));
    }
}

fn normalize_provides(obj: &mut Map<String, Value>) {
    let Some(value) = obj.remove("provides") else {
        return;
    };
    let Some(provides) = value.as_object() else {
        return;
    };

    let mut fixed = Map::new();
    for (pkg, pval) in provides {
        let entry = match pval {
            Value::Object(m) => {
                let mut o = Map::new();
                let file = m.get("file").and_then(scalar_to_string).unwrap_or_default();
                o.insert("file".to_string(), Value::String(file));
                if let Some(ver) = m.get("version").and_then(scalar_to_string) {
                    o.insert("version".to_string(), Value::String(ver));
                }
                Value::Object(o)
            }
            other => match scalar_to_string(other) {
                Some(s) => {
                    let mut o = Map::new();
                    o.insert("file".to_string(), Value::String(s));
                    Value::Object(o)
                }
                None => continue,
            },
        };
        fixed.insert(pkg.clone(), entry);
    }

    if !fixed.is_empty() {
        obj.insert("provides".to_string(), Value::Object(fixed));
    }
}

fn normalize_no_index(obj: &mut Map<String, Value>) {
    let value = obj.remove("no_index").or_else(|| obj.remove("private"));
    obj.remove("private");

    let Some(value) = value else {
        return;
    };
    let Some(m) = value.as_object() else {
        return;
    };

    let mut out = Map::new();
    // (input key, canonical key) — `dir` and `module` are pre-1.3 spellings.
    for (src, dst) in [
        ("file", "file"),
        ("directory", "directory"),
        ("dir", "directory"),
        ("package", "package"),
        ("module", "package"),
        ("namespace", "namespace"),
    ] {
        if let Some(val) = m.get(src) {
            let list = to_string_list(val);
            if list.is_empty() {
                continue;
            }
            let slot = out
                .entry(dst.to_string())
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Some(arr) = slot.as_array_mut() {
                arr.extend(list.into_iter().map(Value::String));
            }
        }
    }

    if !out.is_empty() {
        obj.insert("no_index".to_string(), Value::Object(out));
    }
}

fn normalize_resources(obj: &mut Map<String, Value>) {
    let Some(value) = obj.remove("resources") else {
        return;
    };
    let Some(m) = value.as_object() else {
        return;
    };

    let mut out = Map::new();

    if let Some(h) = m.get("homepage").and_then(first_string) {
        out.insert("homepage".to_string(), Value::String(h));
    }

    if let Some(l) = m.get("license") {
        let list = to_string_list(l);
        if !list.is_empty() {
            out.insert(
                "license".to_string(),
                Value::Array(list.into_iter().map(Value::String).collect()),
            );
        }
    }

    if let Some(bt) = m.get("bugtracker").and_then(normalize_bugtracker) {
        out.insert("bugtracker".to_string(), bt);
    }

    if let Some(repo) = m.get("repository").and_then(normalize_repository) {
        out.insert("repository".to_string(), repo);
    }

    // Preserve custom / capitalised resource keys verbatim.
    for (k, v) in m {
        if matches!(
            k.as_str(),
            "homepage" | "license" | "bugtracker" | "repository"
        ) {
            continue;
        }
        out.insert(k.clone(), v.clone());
    }

    if !out.is_empty() {
        obj.insert("resources".to_string(), Value::Object(out));
    }
}

fn normalize_bugtracker(value: &Value) -> Option<Value> {
    let mut out = Map::new();
    match value {
        Value::String(s) => {
            if s.contains('@') && !s.contains("://") {
                out.insert("mailto".to_string(), Value::String(s.clone()));
            } else {
                out.insert("web".to_string(), Value::String(s.clone()));
            }
        }
        Value::Object(m) => {
            if let Some(web) = m
                .get("web")
                .or_else(|| m.get("url"))
                .and_then(scalar_to_string)
            {
                out.insert("web".to_string(), Value::String(web));
            }
            if let Some(mailto) = m.get("mailto").and_then(scalar_to_string) {
                out.insert("mailto".to_string(), Value::String(mailto));
            }
        }
        _ => {}
    }
    (!out.is_empty()).then_some(Value::Object(out))
}

fn normalize_repository(value: &Value) -> Option<Value> {
    let mut out = Map::new();
    match value {
        Value::String(s) => {
            out.insert("url".to_string(), Value::String(s.clone()));
            if s.starts_with("http://") || s.starts_with("https://") {
                out.insert("web".to_string(), Value::String(s.clone()));
            }
            if let Some(t) = guess_vcs_type(s) {
                out.insert("type".to_string(), Value::String(t.to_string()));
            }
        }
        Value::Object(m) => {
            if let Some(url) = m.get("url").and_then(scalar_to_string) {
                out.insert("url".to_string(), Value::String(url));
            }
            if let Some(web) = m.get("web").and_then(scalar_to_string) {
                out.insert("web".to_string(), Value::String(web));
            }
            let vcs = m
                .get("type")
                .and_then(scalar_to_string)
                .map(|t| t.to_lowercase())
                .or_else(|| {
                    m.get("url")
                        .and_then(scalar_to_string)
                        .as_deref()
                        .and_then(guess_vcs_type)
                        .map(str::to_string)
                });
            if let Some(vcs) = vcs {
                out.insert("type".to_string(), Value::String(vcs));
            }
        }
        _ => {}
    }
    (!out.is_empty()).then_some(Value::Object(out))
}

fn guess_vcs_type(s: &str) -> Option<&'static str> {
    let s = s.to_lowercase();
    if s.contains("git") {
        Some("git")
    } else if s.contains("svn") || s.contains("subversion") {
        Some("svn")
    } else if s.contains("mercurial") || s.contains("/hg") || s.ends_with("hg") {
        Some("hg")
    } else if s.contains("bzr") || s.contains("bazaar") {
        Some("bzr")
    } else if s.contains("darcs") {
        Some("darcs")
    } else if s.contains("cvs") {
        Some("cvs")
    } else {
        None
    }
}

fn normalize_dynamic_config(obj: &mut Map<String, Value>) {
    let value = match obj.get("dynamic_config") {
        None | Some(Value::Null) => true,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::String(s)) => {
            !matches!(s.trim().to_lowercase().as_str(), "" | "0" | "false" | "no")
        }
        Some(_) => true,
    };
    obj.insert("dynamic_config".to_string(), Value::Bool(value));
}

fn normalize_release_status(obj: &mut Map<String, Value>) {
    let status = match obj.get("release_status").and_then(scalar_to_string) {
        Some(s) => {
            let s = s.trim().to_lowercase();
            match s.as_str() {
                "stable" | "testing" | "unstable" => s,
                _ => derive_release_status(obj),
            }
        }
        None => derive_release_status(obj),
    };
    obj.insert("release_status".to_string(), Value::String(status));
}

/// A version with an underscore or a `-TRIAL` suffix denotes a developer
/// release, i.e. `testing`. Everything else defaults to `stable`.
fn derive_release_status(obj: &Map<String, Value>) -> String {
    let version = obj.get("version").and_then(Value::as_str).unwrap_or("");
    let lower = version.to_lowercase();
    if version.contains('_') || lower.contains("-trial") {
        "testing".to_string()
    } else {
        "stable".to_string()
    }
}

// --- prereq helpers -------------------------------------------------------

/// Merge a legacy dependency map into `prereqs` at `phase`/`rel` without
/// overwriting entries already present.
fn merge_deps(prereqs: &mut Map<String, Value>, phase: &str, rel: &str, value: &Value) {
    let deps = to_dep_map(value);
    if deps.is_empty() {
        return;
    }

    let phase_slot = prereqs
        .entry(phase.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(phase_map) = phase_slot.as_object_mut() else {
        return;
    };
    let rel_slot = phase_map
        .entry(rel.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(rel_map) = rel_slot.as_object_mut() else {
        return;
    };
    for (module, version) in deps {
        rel_map
            .entry(module)
            .or_insert_with(|| Value::String(version));
    }
}

/// Force `prereqs` into the strict `{ phase: { relationship: { module:
/// version-string } } }` shape, dropping anything that does not fit.
fn sanitize_prereqs(prereqs: &mut Map<String, Value>) {
    prereqs.retain(|_phase, pv| {
        let Some(phase_map) = pv.as_object_mut() else {
            return false;
        };
        phase_map.retain(|_rel, rv| {
            let Some(rel_map) = rv.as_object_mut() else {
                return false;
            };
            let cleaned: Map<String, Value> = rel_map
                .iter()
                .filter_map(|(k, v)| scalar_to_string(v).map(|s| (k.clone(), Value::String(s))))
                .collect();
            *rel_map = cleaned;
            !rel_map.is_empty()
        });
        !phase_map.is_empty()
    });
}

// --- value helpers ------------------------------------------------------

fn scalar_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "1".to_string() } else { "0".to_string() }),
        _ => None,
    }
}

fn to_string_list(v: &Value) -> Vec<String> {
    match v {
        Value::Array(a) => a.iter().filter_map(scalar_to_string).collect(),
        other => scalar_to_string(other).into_iter().collect(),
    }
}

fn first_string(v: &Value) -> Option<String> {
    match v {
        Value::Array(a) => a.iter().find_map(scalar_to_string),
        other => scalar_to_string(other),
    }
}

fn to_dep_map(v: &Value) -> Vec<(String, String)> {
    match v {
        Value::Object(m) => m
            .iter()
            .map(|(k, val)| {
                (
                    k.clone(),
                    scalar_to_string(val).unwrap_or_else(|| "0".to_string()),
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn value_into_object(v: Value) -> Option<Map<String, Value>> {
    match v {
        Value::Object(m) => Some(m),
        _ => None,
    }
}
