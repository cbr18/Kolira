//! Shared config parsing/patching for Inspector and app-theme operations.
use crate::field;
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::{fs, path::Path};

pub fn ini_parse(text: &str) -> Value {
    let mut result = json!({});
    let mut group = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            group = line[1..line.len() - 1].trim().into();
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if !result[&group].is_object() {
                result[&group] = json!({});
            }
            result[&group][key.trim()] = json!(value.trim());
        }
    }
    result
}

pub fn coerce(old: &Value, raw: &str) -> Result<Value> {
    Ok(match old {
        Value::Bool(_) => match raw.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => json!(true),
            "false" | "0" | "no" | "off" => json!(false),
            _ => bail!("expected true/false, got {raw:?}"),
        },
        Value::Number(n) if n.is_i64() || n.is_u64() => json!(raw.trim().parse::<f64>()? as i64),
        Value::Number(_) => json!(raw.trim().parse::<f64>()?),
        Value::Array(_) => serde_json::from_str::<Value>(raw)
            .ok()
            .filter(Value::is_array)
            .unwrap_or_else(|| json!(raw)),
        Value::Null => serde_json::from_str(raw.trim()).unwrap_or_else(|_| json!(raw)),
        _ => json!(raw),
    })
}

pub fn toml_set(text: &str, key: &str, value: &Value, section: &str) -> Result<String> {
    field::toml_set(text, key, value, section)
}
pub fn toml_get(text: &str, key: &str, path: &[String]) -> Option<Value> {
    field::toml_get(text, key, path)
}
pub fn generic_toml(text: &str, key: &str, raw: &str) -> Result<(String, Value)> {
    let (section, leaf) = key.rsplit_once('/').unwrap_or(("", key));
    let dotted = section.replace('/', ".");
    let header = if section.is_empty() {
        ""
    } else if text.lines().any(|l| l.trim() == format!("[{section}]")) {
        section
    } else if text.lines().any(|l| l.trim() == format!("[{dotted}]")) {
        dotted.as_str()
    } else if section.contains('/') {
        bail!("nested TOML table has no [section] header: {key}")
    } else {
        section
    };
    let path: Vec<String> = if section.is_empty() {
        vec![]
    } else {
        section.split('/').map(str::to_string).collect()
    };
    let old = toml_get(text, leaf, &path).unwrap_or(Value::Null);
    Ok((toml_set(text, leaf, &coerce(&old, raw)?, header)?, old))
}

pub fn strip_jsonc(text: &str) -> String {
    let mut out = String::new();
    let mut it = text.chars().peekable();
    let mut quoted = false;
    let mut escaped = false;
    while let Some(c) = it.next() {
        if quoted {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                quoted = false;
            }
            continue;
        }
        if c == '"' {
            quoted = true;
            out.push(c);
        } else if c == '/' && it.peek() == Some(&'/') {
            it.next();
            for c in it.by_ref() {
                if c == '\n' {
                    out.push(c);
                    break;
                }
            }
        } else if c == '/' && it.peek() == Some(&'*') {
            it.next();
            let mut previous = ' ';
            for c in it.by_ref() {
                if c == '\n' {
                    out.push(c);
                }
                if previous == '*' && c == '/' {
                    break;
                }
                previous = c;
            }
        } else {
            out.push(c);
        }
    }
    out
}
pub fn strip_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let (mut quoted, mut escaped) = (false, false);
    for (i, &c) in chars.iter().enumerate() {
        if !quoted
            && c == ','
            && chars[i + 1..]
                .iter()
                .find(|c| !c.is_whitespace())
                .is_some_and(|c| *c == '}' || *c == ']')
        {
            continue;
        }
        out.push(c);
        if quoted {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                quoted = false;
            }
        } else if c == '"' {
            quoted = true;
        }
    }
    out
}
pub fn json_read(text: &str, path: &[String]) -> Option<Value> {
    field::json_get(text, path)
}
pub fn userjs_read(text: &str, key: &str) -> Option<Value> {
    field::userjs_get(text, key)
}
pub fn userjs_set(text: &str, key: &str, value: &Value) -> String {
    field::userjs_set(text, key, value)
}

pub fn plan(path: &Path, key: &str, raw: &str) -> Result<(String, Value)> {
    if !path.is_file() {
        bail!("File not found: {}", path.display());
    }
    let suffix = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if unsupported(&suffix) {
        bail!("Editing .{suffix} files is not supported");
    }
    if path.file_name().is_some_and(|n| n == "prefs.js") {
        bail!("Edit user.js instead of prefs.js");
    }
    let text = String::from_utf8_lossy(&fs::read(path)?).into_owned();
    match suffix.as_str() {
        "json" | "jsonc" => {
            let parts: Vec<String> = key
                .split('/')
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect();
            let old = json_read(&text, &parts).unwrap_or(Value::Null);
            Ok((field::json_set(&text, &parts, &coerce(&old, raw)?)?, old))
        }
        "toml" => generic_toml(&text, key, raw),
        "js" => {
            let old = userjs_read(&text, key).unwrap_or(Value::Null);
            Ok((userjs_set(&text, key, &coerce(&old, raw)?), old))
        }
        _ => {
            let (group, leaf) = key.split_once('/').unwrap_or(("", key));
            let old = field::ini_get(&text, group, leaf)
                .map(Value::from)
                .unwrap_or(Value::Null);
            if group.is_empty() && old.is_null() {
                if let Some(value) = field::kv_get(&text, leaf) {
                    return Ok((field::kv_set(&text, leaf, raw), json!(value)));
                }
            }
            Ok((field::set_ini_text(&text, group, leaf, raw), old))
        }
    }
}

pub fn unsupported(suffix: &str) -> bool {
    matches!(
        suffix,
        "css"
            | "scss"
            | "png"
            | "jpg"
            | "jpeg"
            | "webp"
            | "bmp"
            | "gif"
            | "svg"
            | "pdf"
            | "zip"
            | "ttf"
            | "otf"
    )
}
fn display(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}
fn flatten(value: &Value, prefix: &str, out: &mut Vec<Value>, limit: usize) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if out.len() >= limit {
                    break;
                }
                let name = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}/{key}")
                };
                flatten(value, &name, out, limit);
            }
        }
        _ => {
            if out.len() < limit {
                out.push(json!({"key":prefix,"value":display(value)}));
            }
        }
    }
}
pub fn params(path: &Path, limit: usize) -> Option<Value> {
    let suffix = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if unsupported(&suffix) {
        return None;
    }
    let meta = fs::metadata(path).ok()?;
    if meta.len() > 512 * 1024 {
        return Some(json!({"params":[],"truncated":false,"note":"file too large"}));
    }
    let text = String::from_utf8_lossy(&fs::read(path).ok()?).into_owned();
    let mut out = vec![];
    let mut note = "";
    match suffix.as_str() {
        "json" | "jsonc" => {
            match serde_json::from_str::<Value>(&strip_trailing_commas(&strip_jsonc(&text))) {
                Ok(value) => flatten(&value, "", &mut out, limit),
                Err(_) => note = "parse error",
            }
        }
        "toml" => match text
            .parse::<toml::Value>()
            .ok()
            .and_then(|v| serde_json::to_value(v).ok())
        {
            Some(value) => flatten(&value, "", &mut out, limit),
            None => note = "parse error",
        },
        "js" => {
            for (key, value) in crate::config_parser::parse_user_js(path).unwrap_or_default() {
                if out.len() >= limit {
                    break;
                }
                out.push(json!({"key":key,"value":value}));
            }
        }
        _ => {
            let mut group = "";
            for line in text.lines() {
                if out.len() >= limit {
                    break;
                }
                let line = line.trim();
                if line.is_empty() || line.starts_with(['#', ';', '!']) {
                    continue;
                }
                if let Some(rest) = line.strip_prefix('[') {
                    if let Some((name, _)) = rest.split_once(']') {
                        group = name.trim();
                        continue;
                    }
                }
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    if !key.is_empty() {
                        out.push(json!({"key":if group.is_empty(){key.into()}else{format!("{group}/{key}")},"value":value.trim()}));
                    }
                }
            }
            if out.is_empty() {
                for line in text.lines() {
                    if out.len() >= limit {
                        break;
                    }
                    let line = line.trim();
                    if line.is_empty() || line.starts_with(['#', ';']) {
                        continue;
                    }
                    let mut parts = line.splitn(2, char::is_whitespace);
                    out.push(json!({"key":parts.next().unwrap_or(""),"value":parts.next().unwrap_or("").trim()}));
                }
            }
        }
    }
    if out.is_empty() && note.is_empty() {
        None
    } else {
        Some(json!({"truncated":out.len()>=limit,"params":out,"note":note}))
    }
}

pub fn rpc(method: &str, args: &Value) -> Result<Value> {
    use crate::gui_api::str_arg as s;
    let text = args["text"].as_str().unwrap_or("");
    let key = args["key"].as_str().unwrap_or("");
    let group = args["group"].as_str().unwrap_or("");
    let path: Vec<String> = args["path"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Ok(match method {
        "ini_parse" => ini_parse(text),
        "ini_get" => json!(field::ini_get(text, group, key)),
        "ini_set" => json!(field::set_ini_text(text, group, key, s(args, "value")?)),
        "json_get" => json!(json_read(text, &path)),
        "json_set" => json!(field::json_set(text, &path, &args["value"])?),
        "toml_get" => json!(toml_get(text, key, &path)),
        "toml_set" => json!(toml_set(text, key, &args["value"], group)?),
        "generic_toml" => {
            let (t, v) = generic_toml(text, key, s(args, "value")?)?;
            json!([t, v])
        }
        "kv_get" => json!(field::kv_get(text, key)),
        "kv_set" => json!(field::kv_set(text, key, s(args, "value")?)),
        "userjs_get" => json!(userjs_read(text, key)),
        "userjs_set" => json!(userjs_set(text, key, &args["value"])),
        "coerce" => coerce(&args["old"], s(args, "value")?)?,
        "strip_jsonc" => json!(strip_jsonc(text)),
        "strip_trailing_commas" => json!(strip_trailing_commas(text)),
        "describe" => {
            let file = Path::new(s(args, "file")?);
            if !file.is_file() {
                Value::Null
            } else {
                let mut parsed = params(file, args["limit"].as_u64().unwrap_or(300) as usize)
                    .unwrap_or_else(
                        || json!({"params":[],"truncated":false,"note":"unsupported format"}),
                    );
                parsed["editable"] = json!(
                    parsed["note"] != "parse error"
                        && parsed["note"] != "unsupported format"
                        && file.file_name().is_none_or(|n| n != "prefs.js")
                );
                parsed
            }
        }
        "params" => params(
            Path::new(s(args, "file")?),
            args["limit"].as_u64().unwrap_or(300) as usize,
        )
        .unwrap_or(Value::Null),
        "plan" => {
            let (t, v) = plan(Path::new(s(args, "file")?), key, s(args, "value")?)?;
            json!([t, v])
        }
        "positions" => {
            let positions: Vec<Value> = field::json_positions(text)
                .into_iter()
                .map(|(p, (start, end))| json!([p, start, end]))
                .collect();
            json!(positions)
        }
        _ => return Err(anyhow!("unknown config operation {method}")),
    })
}
