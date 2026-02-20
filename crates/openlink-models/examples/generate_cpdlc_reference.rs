use serde::Deserialize;
use std::{env, fs, path::PathBuf};

#[derive(Deserialize)]
struct Catalog {
    schema_version: String,
    generated_at_utc: String,
    messages: Vec<CatalogMessage>,
}

#[derive(Deserialize, Clone)]
struct CatalogMessage {
    id: String,
    direction: String,
    template: String,
    args: Vec<String>,
    response_attr: String,
    fans: bool,
    atn_b1: bool,
    is_standby: bool,
    is_closing_response: bool,
    constrained_closing_replies: Vec<String>,
}

fn numeric_id(id: &str) -> u16 {
    id.chars()
        .skip_while(|c| !c.is_ascii_digit())
        .collect::<String>()
        .parse::<u16>()
        .unwrap_or(u16::MAX)
}

fn fmt_bool(v: bool) -> &'static str {
    if v { "Yes" } else { "No" }
}

fn fmt_args(args: &[String]) -> String {
    if args.is_empty() {
        "-".to_string()
    } else {
        args.join(", ")
    }
}

fn fmt_replies(ids: &[String]) -> String {
    if ids.is_empty() {
        "-".to_string()
    } else {
        ids.join(", ")
    }
}

fn render_section(title: &str, mut messages: Vec<CatalogMessage>) -> String {
    messages.sort_by_key(|m| numeric_id(&m.id));

    let mut out = String::new();
    out.push_str(&format!("## {title}\n\n"));
    out.push_str("| ID | Template | Args | Resp | Closing | Standby | Constrained replies | FANS | ATN B1 |\n");
    out.push_str("|---|---|---|---|---|---|---|---|---|\n");

    for m in messages {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            m.id,
            m.template.replace('|', "\\|"),
            fmt_args(&m.args).replace('|', "\\|"),
            m.response_attr,
            fmt_bool(m.is_closing_response),
            fmt_bool(m.is_standby),
            fmt_replies(&m.constrained_closing_replies),
            fmt_bool(m.fans),
            fmt_bool(m.atn_b1)
        ));
    }

    out.push('\n');
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("spec/cpdlc/catalog.v1.json"));

    let output = env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/sdk/reference/cpdlc-messages.md"));

    let catalog: Catalog = serde_json::from_slice(&fs::read(&input)?)?;

    let uplink: Vec<_> = catalog
        .messages
        .iter()
        .filter(|m| m.direction == "Uplink")
        .cloned()
        .collect();

    let downlink: Vec<_> = catalog
        .messages
        .iter()
        .filter(|m| m.direction == "Downlink")
        .cloned()
        .collect();

    let mut doc = String::new();
    doc.push_str("# CPDLC Message Reference\n\n");
    doc.push_str(&format!(
        "Generated from catalog `{}` at `{}`.\n\n",
        catalog.schema_version, catalog.generated_at_utc
    ));
    doc.push_str(&render_section("Uplink messages (UM)", uplink));
    doc.push_str(&render_section("Downlink messages (DM)", downlink));

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, doc)?;
    println!("reference generated to {}", output.display());
    Ok(())
}
