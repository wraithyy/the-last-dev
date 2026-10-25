//! The Ilmari copy pass (docs/ILMARI_1_0.md D20): every spoken line of the finale's content is
//! short enough for a speech bubble, plain ASCII, free of real brands and people, uses only
//! templates the sim fills, and is not said twice by the same speaker.
#![allow(clippy::expect_used)]

use ron::Value;
use std::collections::BTreeMap;

/// A bubble is 300 px wide, three lines of the bubble font: about this many characters.
const BUBBLE_CHARS: usize = 230;
/// Templates the sim, the client or the content loader fill; anything else would reach a player raw.
const TEMPLATES: &[&str] = &[
    "{stamp}",
    "{name}",
    "{subject}",
    "{player}",
    "{slot}",
    "{minutes}",
    "{role}",
    "{progress}",
];
/// Real companies, products and people the satire must not name (whole words, any case).
const REAL: &[&str] = &[
    "openai",
    "chatgpt",
    "gpt",
    "google",
    "microsoft",
    "amazon",
    "facebook",
    "nvidia",
    "anthropic",
    "claude",
    "copilot",
    "gemini",
    "linkedin",
    "slack",
    "jira",
    "github",
    "gitlab",
    "salesforce",
    "oracle",
    "ibm",
    "sap",
    "tesla",
    "twitter",
    "tiktok",
    "netflix",
    "uber",
    "zoom",
    "musk",
    "altman",
    "zuckerberg",
    "bezos",
    "nadella",
    "pichai",
    "amodei",
    "sharepoint",
];

struct Line {
    owner: String,
    text: String,
}

fn load() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/game.ron");
    ron::from_str(&std::fs::read_to_string(path).expect("content")).expect("content parses")
}

fn field<'a>(m: &'a ron::Map, name: &str) -> Option<&'a Value> {
    m.iter()
        .find(|(k, _)| matches!(k, Value::String(s) if s == name))
        .map(|(_, v)| v)
}

fn strings(v: &Value, owner: &str, out: &mut Vec<Line>) {
    match v {
        Value::String(s) => out.push(Line {
            owner: owner.to_string(),
            text: s.clone(),
        }),
        Value::Seq(xs) => xs.iter().for_each(|x| strings(x, owner, out)),
        Value::Option(Some(x)) => strings(x, owner, out),
        Value::Map(m) => m.iter().for_each(|(_, x)| strings(x, owner, out)),
        _ => {}
    }
}

/// Every string below a field named `*yells`, `*_yell`, `line`, `marie_*` or `talk` of Ilmari content: a
/// map whose `key` starts with `il_`, or anything under the top-level `ilmari` block.
fn walk(v: &Value, owner: &str, ilmari: bool, out: &mut Vec<Line>) {
    match v {
        Value::Map(m) => {
            let key = match field(m, "key") {
                Some(Value::String(k)) => k.clone(),
                _ => owner.to_string(),
            };
            let ilmari = ilmari || key.starts_with("il_");
            for (k, x) in m.iter() {
                let Value::String(name) = k else { continue };
                let speaks = name.ends_with("yells")
                    || name.ends_with("_yell")
                    || name == "talk"
                    || name == "line"
                    || name.starts_with("marie_");
                if ilmari && speaks {
                    strings(x, &key, out);
                } else if name == "ilmari" && owner.is_empty() {
                    walk(x, "ilmari", true, out);
                } else {
                    walk(x, &key, ilmari, out);
                }
            }
        }
        Value::Seq(xs) => xs.iter().for_each(|x| walk(x, owner, ilmari, out)),
        Value::Option(Some(x)) => walk(x, owner, ilmari, out),
        _ => {}
    }
}

fn lines() -> Vec<Line> {
    let mut out = Vec::new();
    walk(&load(), "", false, &mut out);
    out
}

fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
}

#[test]
fn the_scan_finds_the_finales_voice() {
    let l = lines();
    assert!(l.len() >= 150, "{} lines found", l.len());
    for owner in ["il_cio", "il_ilmari", "il_marie_core"] {
        assert!(l.iter().any(|x| x.owner == owner), "{owner} has lines");
    }
}

#[test]
fn every_yell_fits_a_bubble_is_plain_ascii_and_names_nobody_real() {
    let mut bad = Vec::new();
    for l in lines() {
        let t = &l.text;
        if t.is_empty() || t.len() > BUBBLE_CHARS {
            bad.push(format!("{}: length {}: {t}", l.owner, t.len()));
        }
        if !t.is_ascii() {
            bad.push(format!("{}: not ASCII: {t}", l.owner));
        }
        if let Some(w) = words(t).find(|w| REAL.contains(&w.as_str())) {
            bad.push(format!("{}: real name {w}: {t}", l.owner));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn every_template_in_a_yell_is_one_the_game_fills() {
    let mut bad = Vec::new();
    for l in lines() {
        let mut rest = l.text.as_str();
        while let Some(i) = rest.find('{') {
            let Some(j) = rest[i..].find('}') else {
                bad.push(format!("{}: unclosed template: {}", l.owner, l.text));
                break;
            };
            let t = &rest[i..=i + j];
            if !TEMPLATES.contains(&t) {
                bad.push(format!("{}: unknown template {t}: {}", l.owner, l.text));
            }
            rest = &rest[i + j..];
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn no_list_repeats_itself_no_speaker_says_a_line_three_times_or_opens_six_alike() {
    let mut by_owner: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for l in lines() {
        by_owner.entry(l.owner).or_default().push(l.text);
    }
    let mut bad = Vec::new();
    for (owner, texts) in &by_owner {
        // A Story and a Full variant of one ability may share a line; a third copy is a rut.
        let mut count: BTreeMap<&String, usize> = BTreeMap::new();
        for t in texts {
            *count.entry(t).or_default() += 1;
        }
        for (t, n) in count.iter().filter(|(_, n)| **n >= 3) {
            bad.push(format!("{owner}: said {n} times: {t}"));
        }
        let mut openers: BTreeMap<String, usize> = BTreeMap::new();
        for t in texts {
            let o: Vec<String> = words(t).take(3).collect();
            if o.len() == 3 {
                *openers.entry(o.join(" ")).or_default() += 1;
            }
        }
        for (o, n) in openers.iter().filter(|(_, n)| **n >= 6) {
            bad.push(format!("{owner}: {n} lines open with \"{o}\""));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The same string twice inside one list is a typo, not a variant.
fn lists(v: &Value, out: &mut Vec<Vec<String>>) {
    match v {
        Value::Seq(xs) => {
            let own: Vec<String> = xs
                .iter()
                .filter_map(|x| match x {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect();
            if own.len() > 1 {
                out.push(own);
            }
            xs.iter().for_each(|x| lists(x, out));
        }
        Value::Map(m) => m.iter().for_each(|(_, x)| lists(x, out)),
        Value::Option(Some(x)) => lists(x, out),
        _ => {}
    }
}

#[test]
fn no_string_list_of_the_finale_repeats_a_line() {
    let ilmari: std::collections::BTreeSet<String> = lines().into_iter().map(|l| l.text).collect();
    let mut all = Vec::new();
    lists(&load(), &mut all);
    let mut bad = Vec::new();
    for list in all.iter().filter(|l| l.iter().any(|t| ilmari.contains(t))) {
        let mut seen = std::collections::BTreeSet::new();
        for t in list.iter().filter(|t| t.len() > 12) {
            if !seen.insert(t) {
                bad.push(t.clone());
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
