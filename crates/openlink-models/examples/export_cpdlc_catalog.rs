use chrono::Utc;
use openlink_models::{
    closes_dialogue_response_elements, constrained_closing_reply_ids, ArgType, CpdlcResponseIntent,
    MessageDirection, MessageElement, ResponseAttribute, MESSAGE_REGISTRY,
};
use serde::Serialize;
use std::{env, fs, path::PathBuf};

#[derive(Serialize)]
struct Catalog {
    schema_version: String,
    generated_at_utc: String,
    messages: Vec<CatalogMessage>,
}

#[derive(Serialize)]
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
    short_response_intents: Vec<ShortResponseIntent>,
}

#[derive(Serialize)]
struct ShortResponseIntent {
    intent: String,
    label: String,
    uplink_id: String,
    downlink_id: String,
}

fn arg_name(arg: ArgType) -> String {
    format!("{arg:?}")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from("spec/cpdlc/catalog.v1.json")
    });

    let messages = MESSAGE_REGISTRY
        .iter()
        .map(|def| {
            let is_standby = matches!(def.id, "DM2" | "UM1" | "UM2");
            let is_closing_response = closes_dialogue_response_elements(&[MessageElement::new(def.id, vec![])]);
            let intents = CpdlcResponseIntent::for_attribute(def.response_attr)
                .into_iter()
                .map(|intent| ShortResponseIntent {
                    intent: format!("{intent:?}"),
                    label: intent.label().to_string(),
                    uplink_id: intent.uplink_id().to_string(),
                    downlink_id: intent.downlink_id().to_string(),
                })
                .collect();

            CatalogMessage {
                id: def.id.to_string(),
                direction: match def.direction {
                    MessageDirection::Uplink => "Uplink".to_string(),
                    MessageDirection::Downlink => "Downlink".to_string(),
                },
                template: def.template.to_string(),
                args: def.args.iter().copied().map(arg_name).collect(),
                response_attr: match def.response_attr {
                    ResponseAttribute::WU => "WU",
                    ResponseAttribute::AN => "AN",
                    ResponseAttribute::R => "R",
                    ResponseAttribute::Y => "Y",
                    ResponseAttribute::N => "N",
                    ResponseAttribute::NE => "NE",
                }
                .to_string(),
                fans: def.fans,
                atn_b1: def.atn_b1,
                is_standby,
                is_closing_response,
                constrained_closing_replies: constrained_closing_reply_ids(def.id)
                    .iter()
                    .map(|id| (*id).to_string())
                    .collect(),
                short_response_intents: intents,
            }
        })
        .collect();

    let catalog = Catalog {
        schema_version: "cpdlc-catalog.v1".to_string(),
        generated_at_utc: Utc::now().to_rfc3339(),
        messages,
    };

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&out, serde_json::to_string_pretty(&catalog)?)?;
    println!("catalog exported to {}", out.display());
    Ok(())
}
