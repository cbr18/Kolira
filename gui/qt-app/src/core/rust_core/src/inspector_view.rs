//! Installed choices and validation shared by Inspector and package validation.
use crate::{
    field::{self, Base},
    paths,
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
fn entries(path: &Path) -> Vec<PathBuf> {
    let mut out: Vec<_> = fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    out.sort();
    out
}
fn choice(value: impl AsRef<str>) -> Value {
    json!({"value":value.as_ref(),"label":value.as_ref()})
}
fn directories(roots: &[(&str, &str)]) -> Vec<Value> {
    roots
        .iter()
        .flat_map(|(root, marker)| {
            entries(&paths::live_path(root))
                .into_iter()
                .filter(move |p| p.is_dir() && p.join(marker).exists())
                .map(|p| choice(p.file_name().unwrap().to_string_lossy()))
        })
        .collect()
}
fn files(roots: &[&str], suffix: &str) -> Vec<Value> {
    roots
        .iter()
        .flat_map(|root| {
            entries(&paths::live_path(root))
                .into_iter()
                .filter(move |p| p.is_file() && p.to_string_lossy().ends_with(suffix))
                .map(move |p| {
                    let name = p.file_name().unwrap().to_string_lossy();
                    choice(name.trim_end_matches(suffix))
                })
        })
        .collect()
}
pub fn choices(id: &str) -> Result<Vec<Value>> {
    let f = field::field(id).context("Unknown field")?;
    let current = field::read_raw(f, &Base::Live)
        .map(|v| field::value_str(&v))
        .unwrap_or_default();
    let mut values = match f.choices.as_str() {
        "color_schemes" => files(
            &[".local/share/color-schemes", "/usr/share/color-schemes"],
            ".colors",
        ),
        "plasma_themes" => {
            let mut v = directories(&[
                (".local/share/plasma/desktoptheme", "metadata.json"),
                ("/usr/share/plasma/desktoptheme", "metadata.json"),
            ]);
            v.push(choice("default"));
            v
        }
        "look_and_feel" => directories(&[
            (".local/share/plasma/look-and-feel", "contents"),
            ("/usr/share/plasma/look-and-feel", "contents"),
        ]),
        "icon_themes" => directories(&[
            (".local/share/icons", "index.theme"),
            (".icons", "index.theme"),
            ("/usr/share/icons", "index.theme"),
        ]),
        "cursor_themes" => directories(&[
            (".local/share/icons", "cursors"),
            (".icons", "cursors"),
            ("/usr/share/icons", "cursors"),
        ]),
        "gtk_themes" => directories(&[
            (".themes", "gtk-3.0"),
            (".local/share/themes", "gtk-3.0"),
            ("/usr/share/themes", "gtk-3.0"),
        ]),
        "btop_themes" => files(&[".config/btop/themes", "/usr/share/btop/themes"], ".theme"),
        "kvantum_themes" => {
            let mut found = vec![];
            for root in [
                paths::live_path(".config/Kvantum"),
                PathBuf::from("/usr/share/Kvantum"),
            ] {
                for path in entries(&root) {
                    if path.is_file() && path.extension().is_some_and(|e| e == "kvconfig") {
                        found.push(choice(path.file_stem().unwrap().to_string_lossy()));
                    } else if path.is_dir() {
                        for file in entries(&path) {
                            if file.is_file() && file.extension().is_some_and(|e| e == "kvconfig") {
                                found.push(choice(file.file_stem().unwrap().to_string_lossy()));
                            }
                        }
                    }
                }
            }
            found
        }
        "widget_styles" => ["Breeze", "kvantum", "Fusion", &current]
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(choice)
            .collect(),
        "decoration_libraries" => [
            "org.kde.breeze",
            "org.kde.klassy",
            "org.kde.kwin.aurorae",
            &current,
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(choice)
        .collect(),
        "decoration_themes" => {
            let mut found = directories(&[
                (".local/share/aurorae/themes", "metadata.json"),
                ("/usr/share/aurorae/themes", "metadata.json"),
            ]);
            found.extend(
                ["Klassy", "Breeze", &current]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .map(choice),
            );
            found
        }
        "border_sizes" => ["None", "Tiny", "Normal", "Large", "Huge"]
            .into_iter()
            .map(choice)
            .collect(),
        "shadow_sizes" => [
            "ShadowNone",
            "ShadowSmall",
            "ShadowMedium",
            "ShadowLarge",
            "ShadowVeryLarge",
        ]
        .into_iter()
        .map(choice)
        .collect(),
        "zen_window_schemes" => vec![
            json!({"value":"0","label":"Dark"}),
            json!({"value":"1","label":"Light"}),
            json!({"value":"2","label":"System"}),
        ],
        "zen_site_schemes" => vec![
            json!({"value":"0","label":"Dark"}),
            json!({"value":"1","label":"Light"}),
            json!({"value":"2","label":"System"}),
            json!({"value":"3","label":"Browser theme"}),
        ],
        "zed_modes" => ["system", "light", "dark"]
            .into_iter()
            .map(choice)
            .collect(),
        _ => vec![],
    };
    let mut seen = BTreeSet::new();
    values.retain(|v| seen.insert(v["value"].as_str().unwrap_or("").to_string()));
    Ok(values)
}
pub fn fonts() -> Vec<String> {
    let mut values = BTreeSet::new();
    if let Some((_, bytes)) = paths::output_bounded_for(
        Command::new("fc-list").args([":", "family"]),
        Duration::from_secs(10),
    ) {
        for value in String::from_utf8_lossy(&bytes).split([',', '\n']) {
            if !value.trim().is_empty() {
                values.insert(value.trim().to_string());
            }
        }
    }
    if values.is_empty() {
        values.extend(
            ["Noto Sans", "DejaVu Sans", "Liberation Sans", "Monospace"].map(str::to_string),
        );
    }
    let mut out: Vec<_> = values.into_iter().collect();
    out.sort_by_key(|v| v.to_lowercase());
    out
}
pub fn validate() -> Result<Value> {
    let mut issues = vec![];
    let families = fonts();
    let asset_choices = [
        "color_schemes",
        "plasma_themes",
        "look_and_feel",
        "icon_themes",
        "cursor_themes",
        "gtk_themes",
        "kvantum_themes",
        "btop_themes",
    ];
    for f in field::registry() {
        let Some(value) = field::read(f, &Base::Live) else {
            continue;
        };
        let text = field::value_str(&value);
        if text.is_empty() {
            continue;
        }
        if asset_choices.contains(&f.choices.as_str())
            && !choices(&f.id)?.iter().any(|c| c["value"] == text)
        {
            issues.push(json!({"severity":if matches!(f.id.as_str(),"kvantum_theme"|"look_and_feel"){"error"}else{"warning"},"field_id":f.id,"message":format!("{}: \"{text}\" is not installed",f.label),"fix_hint":"Choose an installed value"}));
        }
        if f.id == "look_and_feel"
            && (text.contains("Usage:") || text.contains('\n') || text.len() > 80)
        {
            issues.push(json!({"severity":"error","field_id":f.id,"message":"Look & Feel package contains command output instead of a package name","fix_hint":"Choose an installed package (e.g. glass)"}));
        }
        if f.kind == "font" {
            let family = if f.file.contains("gtk") {
                text.rsplit_once(' ').map(|(name, _)| name).unwrap_or(&text)
            } else {
                text.split(',').next().unwrap_or("")
            }
            .trim();
            if !family.is_empty()
                && !families
                    .iter()
                    .any(|v| v.to_lowercase() == family.to_lowercase())
            {
                issues.push(json!({"severity":"warning","field_id":f.id,"message":format!("{}: font \"{family}\" is not installed",f.label),"fix_hint":"Choose an installed font"}));
            }
        }
    }
    if ["blur_enabled", "legacy_blur"]
        .iter()
        .all(|id| field::field(id).and_then(|f| field::read(f, &Base::Live)) == Some(json!(true)))
    {
        issues.push(json!({"severity":"warning","field_id":"blur_enabled","message":"Better Blur DX and the legacy Blur effect are both enabled","fix_hint":"Disable one of the blur providers"}));
    }
    Ok(json!(issues))
}
