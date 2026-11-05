//! Layouts stay the user's: a theme carries the *look* of the desktop shell,
//! never its arrangement.
//!
//! * **`plasma-org.kde.plasma.desktop-appletsrc`** — a theme stores, per
//!   containment, only what identifies it (`plugin`, `formfactor`,
//!   `location`, `lastScreen`, `wallpaperplugin`), the active wallpaper
//!   plugin's settings of each desktop, and the look keys of a few widgets
//!   ([`LOOK_APPLETS`]: the launcher's button icon, Panel Colorizer's
//!   styling). Applying merges just that into the live file: each live
//!   desktop gets the wallpaper of the theme's desktop on the same screen
//!   (else the first), each live launcher / Panel Colorizer gets the look keys
//!   of the theme's n-th widget of that kind. Which widgets exist, where they
//!   sit, their behaviour settings, panels, desktop icons and activities are
//!   never changed.
//! * **`plasmashellrc`** — per panel only `floating` and `panelOpacity`;
//!   thickness, lengths, alignment, offsets and visibility (auto-hide) are the
//!   user's. A live panel takes the theme panel with the same id, else the one
//!   at the same screen edge, else the theme's first panel.
//! * **`kwinrulesrc`** — a rule keeps its match keys and its look properties
//!   ([`RULE_LOOK`]: opacity, border, decoration colour/policy); position,
//!   size, desktops, shortcuts and other behaviour are dropped from the theme
//!   and kept in the live rule. A rule with no look property is not stored.

use std::path::Path;

use crate::appearance::{base_key, parse_ini, render_ini, Section};

/// A Look & Feel package's layout scripts (`contents/layouts/`): never part
/// of a theme — choosing the Global Theme with "Desktop and window layout"
/// would replace the user's panels with the author's.
pub const LAF_LAYOUTS: &str = "contents/layouts";

/// Is `rel` (theme-relative) inside a Look & Feel package's layout scripts?
pub fn is_laf_layout(rel: &Path) -> bool {
    let s = rel.to_string_lossy();
    s.starts_with(".local/share/plasma/look-and-feel/") && s.contains(&format!("/{LAF_LAYOUTS}/"))
}

/// Widgets whose look a theme carries: plugin → keys of `[Configuration][General]`.
pub const LOOK_APPLETS: &[(&str, &[&str])] = &[
    ("luisbocanegra.panel.colorizer", &["globalSettings", "configurationOverrides", "isEnabled", "hideWidget"]),
    ("org.kde.plasma.kickoff", LAUNCHER_LOOK),
    ("org.kde.plasma.simplekickoff", LAUNCHER_LOOK),
    ("org.kde.plasma.kicker", LAUNCHER_LOOK),
    ("org.kde.plasma.kickerdash", LAUNCHER_LOOK),
];
const LAUNCHER_LOOK: &[&str] = &["icon", "useCustomButtonImage", "customButtonImage"];

/// Containment keys a theme keeps (to match containments, not to restore them).
const CONTAINMENT_KEYS: &[&str] = &["plugin", "formfactor", "location", "lastScreen", "wallpaperplugin"];

/// Panel look in `plasmashellrc` `[PlasmaViews][Panel N]`.
pub const PANEL_LOOK: &[&str] = &["floating", "panelOpacity"];

/// Window-rule properties that are look (each with its `…rule` companion).
pub const RULE_LOOK: &[&str] = &["opacityactive", "opacityinactive", "noborder", "decocolor", "decorationpolicy"];
/// Window-rule keys that say which windows a rule matches.
const RULE_MATCH: &[&str] = &[
    "Description", "Enabled", "wmclass", "wmclasscomplete", "wmclassmatch", "title", "titlematch",
    "windowrole", "windowrolematch", "types", "clientmachine", "clientmachinematch",
];

fn segments(group: &str) -> Vec<&str> {
    group.split("][").collect()
}

fn key_of(line: &str) -> Option<&str> {
    base_key(line)
}

fn get<'a>(s: &'a Section, key: &str) -> Option<&'a str> {
    s.lines.iter().find(|l| key_of(l) == Some(key)).and_then(|l| l.split_once('=')).map(|(_, v)| v.trim())
}

/// Set the `allowed` keys of `s` to their lines in `from`: changed in place,
/// new ones appended, ones `from` lacks removed; other keys untouched.
fn set_keys(s: &mut Section, allowed: &[&str], from: &[String]) {
    let wanted: Vec<&String> = from.iter().filter(|l| key_of(l).is_some_and(|k| allowed.contains(&k))).collect();
    let mut used = vec![false; wanted.len()];
    let mut out: Vec<String> = Vec::new();
    for line in &s.lines {
        match key_of(line).filter(|k| allowed.contains(k)) {
            None => out.push(line.clone()),
            Some(k) => {
                if let Some(i) = wanted.iter().position(|w| key_of(w) == Some(k)) {
                    if !used[i] {
                        used[i] = true;
                        out.push(wanted[i].clone());
                    }
                }
            }
        }
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.extend(wanted.iter().zip(&used).filter(|(_, u)| !**u).map(|(w, _)| (*w).clone()));
    s.lines = out;
}

fn look_keys(plugin: &str) -> Option<&'static [&'static str]> {
    LOOK_APPLETS.iter().find(|(p, _)| *p == plugin).map(|(_, k)| *k)
}

fn is_panel(s: &Section) -> bool {
    get(s, "plugin") == Some("org.kde.panel") || get(s, "location").is_some_and(|l| l != "0")
}

/// A desktop's wallpaper plugin (Plasma's default when the key is absent).
fn wallpaper_plugin(s: &Section) -> &str {
    get(s, "wallpaperplugin").filter(|p| !p.is_empty()).unwrap_or("org.kde.image")
}

fn header(group: &str) -> Option<String> {
    Some(format!("[{group}]"))
}

// ─────────────────────────────────────────────────────────────────────────────
// appletsrc
// ─────────────────────────────────────────────────────────────────────────────

/// What a theme keeps of an appletsrc (see the module docs).
pub fn appletsrc_for_theme(text: &str) -> String {
    let sections = parse_ini(text);
    // containment id → (is desktop, wallpaper plugin); applet (c, a) → plugin
    let mut desktops: Vec<(String, String)> = Vec::new();
    let mut applets: Vec<(String, String, String)> = Vec::new();
    for s in &sections {
        let seg = segments(s.group());
        match seg.as_slice() {
            ["Containments", c] if !is_panel(s) => {
                desktops.push(((*c).to_string(), wallpaper_plugin(s).to_string()));
            }
            ["Containments", c, "Applets", a] => {
                if let Some(p) = get(s, "plugin").filter(|p| look_keys(p).is_some()) {
                    applets.push(((*c).to_string(), (*a).to_string(), p.to_string()));
                }
            }
            _ => {}
        }
    }
    let mut out: Vec<Section> = Vec::new();
    for s in &sections {
        let seg = segments(s.group());
        let kept: Option<Vec<String>> = match seg.as_slice() {
            ["Containments", _] => {
                Some(s.lines.iter().filter(|l| key_of(l).is_some_and(|k| CONTAINMENT_KEYS.contains(&k))).cloned().collect())
            }
            ["Containments", c, "Wallpaper", plugin, ..] => desktops
                .iter()
                .any(|(id, wp)| id == c && wp == plugin)
                .then(|| s.lines.iter().filter(|l| key_of(l).is_some()).cloned().collect()),
            ["Containments", c, "Applets", a] => applets
                .iter()
                .any(|(ci, ai, _)| ci == c && ai == a)
                .then(|| s.lines.iter().filter(|l| key_of(l) == Some("plugin")).cloned().collect()),
            ["Containments", c, "Applets", a, "Configuration", "General"] => {
                applets.iter().find(|(ci, ai, _)| ci == c && ai == a).and_then(|(_, _, p)| look_keys(p)).map(|keys| {
                    s.lines.iter().filter(|l| key_of(l).is_some_and(|k| keys.contains(&k))).cloned().collect()
                })
            }
            _ => None,
        };
        if let Some(lines) = kept {
            out.push(Section { header: s.header.clone(), lines });
        }
    }
    if out.is_empty() {
        return String::new();
    }
    render_ini(&out)
}

struct ThemeDesktop {
    screen: String,
    plugin: String,
    /// (group suffix after `Containments][N]`, lines)
    wallpaper: Vec<(String, Vec<String>)>,
}

/// `live` with the theme's look merged in (see the module docs). A theme
/// without an appletsrc look, or an empty live file, leaves `live` as it is.
pub fn appletsrc_merge(theme: &str, live: &str) -> String {
    if live.trim().is_empty() {
        return live.to_string();
    }
    let theme = parse_ini(&appletsrc_for_theme(theme));
    let mut desktops: Vec<(String, ThemeDesktop)> = Vec::new();
    let mut theme_applets: Vec<(String, String, String)> = Vec::new(); // (c, a, plugin)
    for s in &theme {
        let seg = segments(s.group());
        match seg.as_slice() {
            ["Containments", c] if !is_panel(s) => desktops.push((
                (*c).to_string(),
                ThemeDesktop {
                    screen: get(s, "lastScreen").unwrap_or("0").to_string(),
                    plugin: wallpaper_plugin(s).to_string(),
                    wallpaper: Vec::new(),
                },
            )),
            ["Containments", c, "Applets", a] => {
                if let Some(p) = get(s, "plugin") {
                    theme_applets.push(((*c).to_string(), (*a).to_string(), p.to_string()));
                }
            }
            _ => {}
        }
    }
    for s in &theme {
        let group = s.group();
        let seg = segments(group);
        if let ["Containments", c, "Wallpaper", ..] = seg.as_slice() {
            if let Some((_, d)) = desktops.iter_mut().find(|(id, _)| id == c) {
                let suffix = group[format!("Containments][{c}]").len()..].to_string();
                d.wallpaper.push((suffix, s.lines.clone()));
            }
        }
    }
    let theme_look = |c: &str, a: &str| -> Vec<String> {
        let group = format!("Containments][{c}][Applets][{a}][Configuration][General");
        theme.iter().find(|s| s.group() == group).map(|s| s.lines.clone()).unwrap_or_default()
    };

    let mut sections = parse_ini(live);
    let before = render_ini(&sections);

    // Wallpapers: every live desktop takes the theme's desktop on its screen.
    let live_desktops: Vec<(String, String)> = sections
        .iter()
        .filter_map(|s| match segments(s.group()).as_slice() {
            ["Containments", c] if !is_panel(s) => Some(((*c).to_string(), get(s, "lastScreen").unwrap_or("0").to_string())),
            _ => None,
        })
        .collect();
    for (c, screen) in &live_desktops {
        let Some((_, d)) = desktops.iter().find(|(_, d)| &d.screen == screen).or_else(|| desktops.first()) else {
            continue;
        };
        if d.plugin.is_empty() {
            continue;
        }
        let own = format!("Containments][{c}]");
        if let Some(s) = sections.iter_mut().find(|s| s.group() == format!("Containments][{c}")) {
            set_keys(s, &["wallpaperplugin"], &[format!("wallpaperplugin={}", d.plugin)]);
        }
        let prefix = format!("{own}[Wallpaper][{}]", d.plugin);
        sections.retain(|s| !(s.group() == prefix || s.group().starts_with(&format!("{prefix}["))));
        let at = sections.iter().rposition(|s| s.group() == own.trim_end_matches(']') || s.group().starts_with(&own)).map_or(sections.len(), |i| i + 1);
        for (i, (suffix, lines)) in d.wallpaper.iter().enumerate() {
            sections.insert(at + i, Section { header: header(&format!("Containments][{c}]{suffix}")), lines: lines.clone() });
        }
    }

    // Widget look: the n-th live widget of a kind takes the theme's n-th (else first).
    let live_applets: Vec<(String, String, String)> = sections
        .iter()
        .filter_map(|s| match segments(s.group()).as_slice() {
            ["Containments", c, "Applets", a] => get(s, "plugin").map(|p| ((*c).to_string(), (*a).to_string(), p.to_string())),
            _ => None,
        })
        .collect();
    let mut seen: Vec<(String, usize)> = Vec::new();
    for (c, a, plugin) in &live_applets {
        let Some(keys) = look_keys(plugin) else { continue };
        let n = match seen.iter_mut().find(|(p, _)| p == plugin) {
            Some((_, n)) => {
                *n += 1;
                *n
            }
            None => {
                seen.push((plugin.clone(), 0));
                0
            }
        };
        let ours: Vec<&(String, String, String)> = theme_applets.iter().filter(|(_, _, p)| p == plugin).collect();
        let Some((tc, ta, _)) = ours.get(n).or_else(|| ours.first()) else { continue };
        let look = theme_look(tc, ta);
        let group = format!("Containments][{c}][Applets][{a}][Configuration][General");
        match sections.iter_mut().find(|s| s.group() == group) {
            Some(s) => set_keys(s, keys, &look),
            None if !look.is_empty() => {
                let own = format!("Containments][{c}][Applets][{a}]");
                let at = sections.iter().rposition(|s| s.group().starts_with(&own) || s.group() == own.trim_end_matches(']')).map_or(sections.len(), |i| i + 1);
                let mut s = Section { header: header(&group), lines: Vec::new() };
                set_keys(&mut s, keys, &look);
                sections.insert(at, s);
            }
            None => {}
        }
    }

    let after = render_ini(&sections);
    if after == before {
        live.to_string()
    } else {
        after
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// plasmashellrc
// ─────────────────────────────────────────────────────────────────────────────

fn panel_id(group: &str) -> Option<&str> {
    match segments(group).as_slice() {
        ["PlasmaViews", panel] => panel.strip_prefix("Panel "),
        _ => None,
    }
}

/// Panel id → screen edge (`location`) from an appletsrc.
fn panel_locations(appletsrc: Option<&str>) -> Vec<(String, String)> {
    let Some(text) = appletsrc else { return Vec::new() };
    parse_ini(text)
        .iter()
        .filter_map(|s| match segments(s.group()).as_slice() {
            ["Containments", c] if is_panel(s) => Some(((*c).to_string(), get(s, "location").unwrap_or_default().to_string())),
            _ => None,
        })
        .collect()
}

/// What a theme keeps of plasmashellrc: `[PlasmaViews][Panel N]` look keys.
pub fn panel_views_for_theme(text: &str) -> String {
    let out: Vec<Section> = parse_ini(text)
        .into_iter()
        .filter(|s| panel_id(s.group()).is_some())
        .map(|s| Section {
            lines: s.lines.into_iter().filter(|l| key_of(l).is_some_and(|k| PANEL_LOOK.contains(&k))).collect(),
            header: s.header,
        })
        .filter(|s| !s.lines.is_empty())
        .collect();
    if out.is_empty() {
        String::new()
    } else {
        render_ini(&out)
    }
}

/// `live` plasmashellrc with each existing panel's look from the theme.
/// `theme_layout` / `live_layout` (appletsrc texts) let panels with other ids
/// match by screen edge.
pub fn panel_views_merge(theme: &str, live: &str, theme_layout: Option<&str>, live_layout: Option<&str>) -> String {
    if live.trim().is_empty() {
        return live.to_string();
    }
    let theme_panels: Vec<(String, Vec<String>)> = parse_ini(&panel_views_for_theme(theme))
        .into_iter()
        .filter_map(|s| panel_id(s.group()).map(|id| (id.to_string(), s.lines.clone())))
        .collect();
    if theme_panels.is_empty() {
        return live.to_string();
    }
    let theme_edges = panel_locations(theme_layout);
    let live_edges = panel_locations(live_layout);
    let edge = |edges: &[(String, String)], id: &str| edges.iter().find(|(c, _)| c == id).map(|(_, l)| l.clone());

    let mut sections = parse_ini(live);
    let before = render_ini(&sections);
    for s in sections.iter_mut() {
        let Some(id) = panel_id(s.group()).map(str::to_string) else { continue };
        let pick = theme_panels
            .iter()
            .find(|(t, _)| *t == id)
            .or_else(|| {
                let want = edge(&live_edges, &id)?;
                theme_panels.iter().find(|(t, _)| edge(&theme_edges, t).as_deref() == Some(want.as_str()))
            })
            .or_else(|| theme_panels.first());
        if let Some((_, look)) = pick {
            set_keys(s, PANEL_LOOK, look);
        }
    }
    let after = render_ini(&sections);
    if after == before {
        live.to_string()
    } else {
        after
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// kwinrulesrc
// ─────────────────────────────────────────────────────────────────────────────

fn rule_look(key: &str) -> bool {
    RULE_LOOK.iter().any(|p| key == *p || key.strip_suffix("rule") == Some(p))
}

fn rule_kept(key: &str) -> bool {
    rule_look(key) || RULE_MATCH.contains(&key)
}

/// What a theme keeps of kwinrulesrc: rules with a look property, reduced to
/// match keys + look properties; `[General]` lists them.
pub fn kwinrules_for_theme(text: &str) -> String {
    let mut rules: Vec<Section> = Vec::new();
    for s in parse_ini(text) {
        if s.header.is_none() || s.group() == "General" {
            continue;
        }
        if !s.lines.iter().any(|l| key_of(l).is_some_and(rule_look)) {
            continue;
        }
        rules.push(Section {
            lines: s.lines.into_iter().filter(|l| key_of(l).is_some_and(rule_kept)).collect(),
            header: s.header,
        });
    }
    if rules.is_empty() {
        return String::new();
    }
    let ids: Vec<String> = rules.iter().map(|s| s.group().to_string()).collect();
    let mut out = vec![Section { header: Some("[General]".into()), lines: rule_list_lines(&ids, &[]) }];
    out.extend(rules);
    render_ini(&out)
}

/// `[General]` lines listing `ids`, in the format `general` (the live
/// `[General]` lines) uses: KWin ≥ 6.x writes `Order=`, older KWin
/// `count=` + `rules=`; with neither, both are written (unknown keys are ignored).
pub fn rule_list_lines(ids: &[String], general: &[String]) -> Vec<String> {
    let has = |k: &str| general.iter().any(|l| key_of(l) == Some(k));
    let (order, legacy) = match (has("Order"), has("rules") || has("count")) {
        (false, false) => (true, true),
        other => other,
    };
    let mut out = Vec::new();
    if legacy {
        out.push(format!("count={}", ids.len()));
        out.push(format!("rules={}", ids.join(",")));
    }
    if order {
        out.push(format!("Order={}", ids.join(",")));
    }
    out
}

/// For a live rule with the same id: its behaviour keys stay, the match keys
/// and look properties come from the theme.
pub fn kwinrule_lines(theme_lines: &[String], live_lines: &[String]) -> Vec<String> {
    let mut out: Vec<String> = live_lines.iter().filter(|l| !key_of(l).is_some_and(rule_kept)).cloned().collect();
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.extend(theme_lines.iter().filter(|l| key_of(l).is_some_and(rule_kept)).cloned());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const THEME: &str = "\
[ActionPlugins][0]
RightButton;NoModifier=org.kde.contextmenu

[Containments][1]
activityId=abc
formfactor=0
lastScreen=0
location=0
plugin=org.kde.plasma.folder
wallpaperplugin=org.kde.image

[Containments][1][General]
positions={\"secret\":1}

[Containments][1][Wallpaper][org.kde.image][General]
Image=file:///home/author/theme.jpg
FillMode=2

[Containments][1][Wallpaper][org.kde.slideshow][General]
SlidePaths=/home/author/private/

[Containments][2]
formfactor=2
location=4
plugin=org.kde.panel

[Containments][2][Applets][3]
plugin=org.kde.plasma.kickoff

[Containments][2][Applets][3][Configuration][General]
icon=theme-logo
favorites=firefox.desktop

[Containments][2][Applets][4]
plugin=org.kde.plasma.weather

[Containments][2][Applets][4][Configuration][General]
source=bbcukmet|weather|Home Town

[Containments][2][General]
AppletOrder=3;4
";

    const LIVE: &str = "\
[Containments][10]
activityId=mine
formfactor=0
lastScreen=0
location=0
plugin=org.kde.desktopcontainment
wallpaperplugin=org.kde.potd

[Containments][10][General]
positions={\"mine\":1}

[Containments][10][Wallpaper][org.kde.image][General]
Image=file:///home/me/old.jpg

[Containments][20]
formfactor=2
location=3
plugin=org.kde.panel

[Containments][20][Applets][21]
plugin=org.kde.plasma.icontasks

[Containments][20][Applets][22]
plugin=org.kde.plasma.kickoff

[Containments][20][Applets][22][Configuration][General]
favorites=kitty.desktop
icon=start-here

[Containments][20][General]
AppletOrder=22;21
";

    #[test]
    fn laf_layout_scripts_are_not_theme_files() {
        assert!(is_laf_layout(Path::new(".local/share/plasma/look-and-feel/glass/contents/layouts/org.kde.plasma.desktop-layout.js")));
        assert!(!is_laf_layout(Path::new(".local/share/plasma/look-and-feel/glass/contents/defaults")));
        assert!(!is_laf_layout(Path::new(".config/contents/layouts/x")));
    }

    #[test]
    fn appletsrc_theme_keeps_only_the_look() {
        let t = appletsrc_for_theme(THEME);
        for gone in ["activityId", "positions", "secret", "slideshow", "private", "weather", "Home Town", "favorites", "AppletOrder", "ActionPlugins"] {
            assert!(!t.contains(gone), "{gone} stored:\n{t}");
        }
        for kept in ["Image=file:///home/author/theme.jpg", "FillMode=2", "icon=theme-logo", "plugin=org.kde.plasma.kickoff", "location=4"] {
            assert!(t.contains(kept), "{kept} missing:\n{t}");
        }
        // Idempotent: a stored file filters to itself.
        assert_eq!(appletsrc_for_theme(&t), t);
    }

    #[test]
    fn appletsrc_merge_keeps_the_users_layout() {
        let m = appletsrc_merge(THEME, LIVE);
        // look in
        assert!(m.contains("wallpaperplugin=org.kde.image"), "{m}");
        assert!(m.contains("Image=file:///home/author/theme.jpg") && m.contains("FillMode=2"));
        assert!(!m.contains("old.jpg"));
        assert!(m.contains("icon=theme-logo") && !m.contains("icon=start-here"));
        // layout stays
        for kept in ["favorites=kitty.desktop", "AppletOrder=22;21", "org.kde.plasma.icontasks", "location=3", "plugin=org.kde.desktopcontainment", "positions={\"mine\":1}", "activityId=mine"] {
            assert!(m.contains(kept), "{kept} lost:\n{m}");
        }
        for never in ["weather", "Home Town", "firefox.desktop", "secret", "location=4"] {
            assert!(!m.contains(never), "{never} leaked:\n{m}");
        }
        assert!(m.contains("[Containments][10][Wallpaper][org.kde.image][General]"));
        assert!(!m.contains("[Containments][1]"));
        // Applying again changes nothing.
        assert_eq!(appletsrc_merge(THEME, &m), m);
    }

    #[test]
    fn appletsrc_merge_leaves_empty_or_unrelated_live_alone() {
        assert_eq!(appletsrc_merge(THEME, ""), "");
        let live = "[Containments][5]\nformfactor=2\nlocation=4\nplugin=org.kde.panel\n";
        assert_eq!(appletsrc_merge(THEME, live), live);
        assert_eq!(appletsrc_merge("", LIVE), LIVE);
    }

    #[test]
    fn panel_views_take_only_floating_and_opacity() {
        let theme = "[PlasmaViews][Panel 2]\nfloating=1\npanelOpacity=2\npanelVisibility=1\n\n[PlasmaViews][Panel 2][Defaults]\nthickness=60\n";
        assert_eq!(panel_views_for_theme(theme), "[PlasmaViews][Panel 2]\nfloating=1\npanelOpacity=2\n");
        let live = "[PlasmaViews][Panel 20]\nfloating=0\npanelOpacity=1\npanelVisibility=0\n\n[PlasmaViews][Panel 20][Defaults]\nthickness=44\n\n[Updates]\nperformed=x\n";
        let theme_layout = "[Containments][2]\nlocation=4\nplugin=org.kde.panel\n";
        let live_layout = "[Containments][20]\nlocation=4\nplugin=org.kde.panel\n";
        let m = panel_views_merge(theme, live, Some(theme_layout), Some(live_layout));
        assert!(m.contains("floating=1") && m.contains("panelOpacity=2"), "{m}");
        assert!(m.contains("panelVisibility=0") && m.contains("thickness=44") && m.contains("performed=x"), "{m}");
        assert!(!m.contains("thickness=60") && !m.contains("Panel 2]"), "{m}");
        assert_eq!(panel_views_merge(theme, &m, Some(theme_layout), Some(live_layout)), m);
    }

    #[test]
    fn kwinrules_keep_look_and_drop_behaviour() {
        let text = "[General]\ncount=2\nrules=a,b\n\n[a]\nDescription=translucent kitty\nwmclass=kitty\nwmclassmatch=1\nopacityactive=90\nopacityactiverule=2\nposition=10,10\npositionrule=3\n\n[b]\nDescription=size\nsize=800,600\nsizerule=3\n";
        let t = kwinrules_for_theme(text);
        assert!(t.contains("count=1") && t.contains("rules=a") && t.contains("opacityactive=90") && t.contains("wmclass=kitty"), "{t}");
        assert!(!t.contains("position") && !t.contains("[b]") && !t.contains("size="), "{t}");
        assert_eq!(kwinrules_for_theme("[General]\ncount=1\nrules=b\n\n[b]\nsize=1,1\nsizerule=3\n"), "");
        let live = vec!["Description=old".to_string(), "position=5,5".into(), "positionrule=3".into(), "opacityactive=50".into()];
        let theme = vec!["Description=translucent kitty".to_string(), "opacityactive=90".into()];
        let merged = kwinrule_lines(&theme, &live);
        assert_eq!(merged, vec!["position=5,5", "positionrule=3", "Description=translucent kitty", "opacityactive=90"]);
    }
}
