use dioxus::prelude::*;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use crate::i18n::{t, use_locale};
use crate::state::{AppState, AtcLinkedFlight, NatsClients, ReceivedMessage, TabState};
use openlink_models::{
    closes_dialogue_response_elements, find_definition, AcarsEndpointAddress, AcarsMessage,
    CpdlcArgument, CpdlcConnectionPhase, CpdlcMessageType, CpdlcResponseIntent, FlightLevel,
    MessageElement, OpenLinkMessage, ResponseAttribute,
};

fn render_element(element: &MessageElement) -> String {
    find_definition(&element.id)
        .map(|def| def.render(&element.args))
        .unwrap_or_else(|| element.id.clone())
}

fn phase_status_label(phase: CpdlcConnectionPhase) -> &'static str {
    match phase {
        CpdlcConnectionPhase::Connected => "CPDLC CONNECTED",
        CpdlcConnectionPhase::LogonPending => "LOGON REQUEST RECEIVED",
        CpdlcConnectionPhase::LoggedOn => "LOGON ACCEPTED (WAITING LINK)",
        CpdlcConnectionPhase::Terminated => "ACARS ONLY",
    }
}

fn is_logical_ack(msg: &ReceivedMessage) -> bool {
    // Strict protocol-only detection via parsed CPDLC application payload.
    let Some(env) = msg.envelope.as_ref() else {
        // Local synthetic rows may only carry rendered text.
        return msg
            .display_text
            .as_deref()
            .map(|t| t.to_ascii_uppercase().contains("LOGICAL ACKNOWLEDGMENT"))
            .unwrap_or(false);
    };
    let OpenLinkMessage::Acars(acars_env) = &env.payload else {
        return false;
    };
    let AcarsMessage::CPDLC(cpdlc_env) = &acars_env.message;
    let CpdlcMessageType::Application(app) = &cpdlc_env.message else {
        return false;
    };
    openlink_sdk::message_contains_logical_ack(&app.elements)
}

fn local_short_response_intent(msg: &ReceivedMessage) -> Option<CpdlcResponseIntent> {
    let text = msg
        .display_text
        .as_deref()
        .map(|t| t.trim().to_ascii_uppercase())?;
    match text.as_str() {
        "WILCO" => Some(CpdlcResponseIntent::Wilco),
        "UNABLE" => Some(CpdlcResponseIntent::Unable),
        "STANDBY" => Some(CpdlcResponseIntent::Standby),
        "ROGER" => Some(CpdlcResponseIntent::Roger),
        "AFFIRM" => Some(CpdlcResponseIntent::Affirm),
        "NEGATIVE" => Some(CpdlcResponseIntent::Negative),
        _ => None,
    }
}

fn has_lack_for_outgoing(msg: &ReceivedMessage, messages: &[ReceivedMessage]) -> bool {
    if !msg.is_outgoing {
        return false;
    }

    // Strict protocol attribution: a LACK acknowledges exactly one message via MRN -> MIN.
    let Some(min) = msg.min else {
        return false;
    };

    messages.iter().any(|ack| {
        if ack.is_outgoing {
            return false;
        }

        // Prefer protocol-level matching from SDK when a CPDLC application payload is available.
        if let Some(env) = ack.envelope.as_ref() {
            let OpenLinkMessage::Acars(acars_env) = &env.payload else {
                return false;
            };
            let AcarsMessage::CPDLC(cpdlc_env) = &acars_env.message;
            let CpdlcMessageType::Application(app) = &cpdlc_env.message else {
                return false;
            };
            return openlink_sdk::logical_ack_matches_outgoing(min, &app.elements, app.mrn);
        }

        false
    })
}

fn should_track_lack(msg: &ReceivedMessage) -> bool {
    if !msg.is_outgoing || msg.to_callsign.is_none() {
        return false;
    }

    // Never track LACK messages or logon protocol lines.
    let text = msg.display_text.as_deref().unwrap_or_default().to_ascii_uppercase();
    !is_logical_ack(msg) && !text.contains("LOGON")
}

fn is_logon_line(msg: &ReceivedMessage) -> bool {
    let text = msg.display_text.as_deref().unwrap_or_default().to_ascii_uppercase();
    text.contains("LOGON")
}

fn is_closing_response_message(msg: &ReceivedMessage) -> bool {
    let Some(env) = msg.envelope.as_ref() else {
        // Local quick responses are stored without envelope; treat pure short
        // responses as closing, while composed messages remain non-closing.
        return matches!(
            local_short_response_intent(msg),
            Some(
                CpdlcResponseIntent::Wilco
                    | CpdlcResponseIntent::Unable
                    | CpdlcResponseIntent::Roger
                    | CpdlcResponseIntent::Affirm
                    | CpdlcResponseIntent::Negative
            )
        );
    };
    let OpenLinkMessage::Acars(acars_env) = &env.payload else {
        return false;
    };
    let AcarsMessage::CPDLC(cpdlc_env) = &acars_env.message;
    let CpdlcMessageType::Application(app) = &cpdlc_env.message else {
        return false;
    };
    closes_dialogue_response_elements(&app.elements)
}

fn is_standby_message(msg: &ReceivedMessage) -> bool {
    let Some(env) = msg.envelope.as_ref() else {
        // Local UI-outgoing rows are stored without an envelope.
        // Keep standby behavior by recognizing synthetic quick-response labels.
        return msg
            .display_text
            .as_deref()
            .map(|t| t.trim().eq_ignore_ascii_case("STANDBY"))
            .unwrap_or(false);
    };
    let OpenLinkMessage::Acars(acars_env) = &env.payload else {
        return false;
    };
    let AcarsMessage::CPDLC(cpdlc_env) = &acars_env.message;
    let CpdlcMessageType::Application(app) = &cpdlc_env.message else {
        return false;
    };
    app.elements
        .iter()
        .any(|e| matches!(e.id.as_str(), "DM2" | "UM1" | "UM2"))
}

fn response_intents_for_message(msg: &ReceivedMessage) -> Vec<CpdlcResponseIntent> {
    let Some(env) = msg.envelope.as_ref() else {
        return msg
            .response_attr
            .map(openlink_sdk::response_attr_to_intents)
            .unwrap_or_default();
    };
    let OpenLinkMessage::Acars(acars_env) = &env.payload else {
        return msg
            .response_attr
            .map(openlink_sdk::response_attr_to_intents)
            .unwrap_or_default();
    };
    let AcarsMessage::CPDLC(cpdlc_env) = &acars_env.message;
    let CpdlcMessageType::Application(app) = &cpdlc_env.message else {
        return msg
            .response_attr
            .map(openlink_sdk::response_attr_to_intents)
            .unwrap_or_default();
    };

    // Prefer catalog-based element resolution, then enrich with effective attr intents.
    let mut intents = openlink_sdk::choose_short_response_intents(&app.elements);
    if let Some(attr) = msg.response_attr {
        for i in openlink_sdk::response_attr_to_intents(attr) {
            if !intents.contains(&i) {
                intents.push(i);
            }
        }
    }
    intents
}

fn action_btn_class(intent: CpdlcResponseIntent) -> &'static str {
    match intent {
        CpdlcResponseIntent::Unable => "action-btn unable",
        CpdlcResponseIntent::Standby => "action-btn standby",
        _ => "action-btn",
    }
}

fn is_priority_response_intent(intent: CpdlcResponseIntent) -> bool {
    matches!(
        intent,
        CpdlcResponseIntent::Wilco | CpdlcResponseIntent::Unable | CpdlcResponseIntent::Standby
    )
}

fn standby_elapsed_label(ts: chrono::DateTime<chrono::Utc>) -> String {
    let elapsed = chrono::Utc::now() - ts;
    let total = elapsed.num_seconds().max(0);
    let mm = total / 60;
    let ss = total % 60;
    format!("{mm:02}:{ss:02}")
}

const CLOSED_DIALOGUE_RETENTION_SECS: i64 = 60;

fn is_recent_closure_message(msg: &ReceivedMessage) -> bool {
    if !is_closing_response_message(msg) {
        return false;
    }
    let age = chrono::Utc::now() - msg.timestamp;
    age.num_seconds() <= CLOSED_DIALOGUE_RETENTION_SECS
}

fn closing_flag_intent(msg: &ReceivedMessage) -> Option<CpdlcResponseIntent> {
    if !is_closing_response_message(msg) {
        return None;
    }
    if let Some(intent) = local_short_response_intent(msg) {
        return Some(intent);
    }
    let intents = response_intents_for_message(msg);
    [
        CpdlcResponseIntent::Unable,
        CpdlcResponseIntent::Negative,
        CpdlcResponseIntent::Wilco,
        CpdlcResponseIntent::Roger,
        CpdlcResponseIntent::Affirm,
    ]
    .iter()
    .copied()
    .find(|i| intents.contains(i))
}

fn is_positive_closure_intent(intent: CpdlcResponseIntent) -> bool {
    matches!(
        intent,
        CpdlcResponseIntent::Wilco | CpdlcResponseIntent::Roger | CpdlcResponseIntent::Affirm
    )
}

fn find_dialogue_message_by_min<'a>(
    messages: &'a [ReceivedMessage],
    min: u8,
    callsign: &str,
) -> Option<&'a ReceivedMessage> {
    messages
        .iter()
        .find(|m| m.min == Some(min) && !is_logical_ack(m) && !is_logon_line(m) && dialogue_callsign(m).as_deref() == Some(callsign))
}

fn select_flight_by_callsign(tab: &mut TabState, aircraft_callsign: &str) {
    let mut sorted_callsigns: Vec<String> = tab
        .atc_sessions
        .values()
        .filter_map(|s| s.aircraft.as_ref().map(|c| c.to_string()))
        .collect();
    sorted_callsigns.sort();
    sorted_callsigns.dedup();
    if let Some(idx) = sorted_callsigns.iter().position(|c| c == aircraft_callsign) {
        tab.selected_flight_idx = Some(idx);
    }
}

fn find_linked_flight(tab: &TabState, aircraft_callsign: &str) -> Option<AtcLinkedFlight> {
    tab.atc_sessions.values().find_map(|session| {
        let cs = session.aircraft.as_ref()?.to_string();
        if cs != aircraft_callsign {
            return None;
        }
        let aircraft_address = session.aircraft_address.as_ref()?.clone();
        let phase = session
            .active_connection
            .as_ref()
            .map(|c| c.phase)
            .or_else(|| session.inactive_connection.as_ref().map(|c| c.phase))
            .unwrap_or(CpdlcConnectionPhase::Terminated);
        Some(AtcLinkedFlight {
            callsign: cs.clone(),
            aircraft_callsign: cs,
            aircraft_address,
            phase,
        })
    })
}

fn is_dialogue_candidate(msg: &ReceivedMessage) -> bool {
    !is_logical_ack(msg) && !is_logon_line(msg) && msg.min.is_some()
}

/// Returns the aircraft callsign involved in a dialogue message,
/// used to scope MIN/MRN matching per aircraft session.
fn dialogue_callsign(msg: &ReceivedMessage) -> Option<String> {
    if msg.is_outgoing {
        msg.to_callsign.clone()
    } else {
        msg.from_callsign.clone()
    }
}

/// Visual flag displayed on a pending dialogue card.
#[derive(Debug, Clone)]
enum DialogueFlag {
    None,
    Standby(chrono::DateTime<chrono::Utc>),
    Closed {
        intent: CpdlcResponseIntent,
        positive: bool,
    },
    Received(chrono::DateTime<chrono::Utc>),
}

/// Pre-computed dialogue entry for the pending requests queue.
/// Bundles the aircraft identity, actionable MIN, display text, response intents,
/// and visual state — eliminating ad-hoc per-card recomputation in the render loop.
#[derive(Debug, Clone)]
struct PendingDialogue {
    /// Aircraft callsign — the identity anchor for all message targeting.
    aircraft_callsign: String,
    /// MIN of the request to respond to (becomes MRN of the response).
    action_min: Option<u8>,
    /// Human-readable text for the request card.
    display_text: String,
    /// Available response buttons (empty = no action buttons shown).
    response_intents: Vec<CpdlcResponseIntent>,
    /// Whether the original message had ResponseAttribute::Y (controls button layout).
    is_y_response: bool,
    /// Timestamp of the dialogue's latest message.
    timestamp: chrono::DateTime<chrono::Utc>,
    /// CSS class for the card container.
    card_class: &'static str,
    /// Visual flag (standby clock, closure badge, received indicator).
    flag: DialogueFlag,
    /// Sort priority: 0 = needs action, 1 = monitoring, 2 = closing.
    priority: u8,
}

fn build_pending_dialogues(messages: &[ReceivedMessage]) -> Vec<PendingDialogue> {
    #[derive(Debug, Clone)]
    struct DialogueTrack {
        mins: Vec<u8>,
        last_index: usize,
        closed: bool,
    }

    // Scope dialogue tracking by (callsign, root_min) to avoid cross-aircraft MIN collisions.
    let mut min_to_root: HashMap<(String, u8), u8> = HashMap::new();
    let mut dialogues: HashMap<(String, u8), DialogueTrack> = HashMap::new();

    for (idx, msg) in messages.iter().enumerate() {
        if !is_dialogue_candidate(msg) {
            continue;
        }
        let Some(min) = msg.min else { continue };
        let Some(cs) = dialogue_callsign(msg) else {
            continue;
        };

        let root = if let Some(parent) = msg.mrn {
            min_to_root
                .get(&(cs.clone(), parent))
                .copied()
                .unwrap_or(min)
        } else {
            min
        };

        min_to_root.insert((cs.clone(), min), root);

        let key = (cs, root);
        let track = dialogues.entry(key).or_insert(DialogueTrack {
            mins: Vec::new(),
            last_index: idx,
            closed: false,
        });
        if !track.mins.contains(&min) {
            track.mins.push(min);
        }
        track.last_index = idx;

        if is_closing_response_message(msg) {
            track.closed = true;
        }
    }

    let mut out: Vec<PendingDialogue> = Vec::new();

    for d in dialogues.values() {
        let req = &messages[d.last_index];
        if d.closed && !is_recent_closure_message(req) {
            continue;
        }

        let recent_closure = is_recent_closure_message(req);
        let cs = dialogue_callsign(req).unwrap_or_default();

        // Resolve original actionable message.
        let action_source = if is_standby_message(req) || recent_closure {
            req.mrn
                .and_then(|mrn| find_dialogue_message_by_min(messages, mrn, &cs))
        } else if !req.is_outgoing {
            Some(req)
        } else {
            None
        };

        let standby_from_aircraft = !req.is_outgoing && is_standby_message(req);
        let standby_from_atc = req.is_outgoing && is_standby_message(req);
        let closure_intent = if recent_closure {
            closing_flag_intent(req)
        } else {
            None
        };
        let closure_positive = closure_intent
            .map(is_positive_closure_intent)
            .unwrap_or(false);
        let action_min = action_source.and_then(|m| m.min);

        // Compute response intents.
        let is_y_response = matches!(
            action_source.and_then(|m| m.response_attr),
            Some(ResponseAttribute::Y)
        );
        let mut response_intents = action_source
            .map(response_intents_for_message)
            .unwrap_or_default();
        if is_y_response {
            response_intents = vec![CpdlcResponseIntent::Unable, CpdlcResponseIntent::Standby];
        } else if action_source.is_some() && response_intents.is_empty() {
            response_intents = vec![
                CpdlcResponseIntent::Wilco,
                CpdlcResponseIntent::Unable,
                CpdlcResponseIntent::Standby,
            ];
        }
        if standby_from_atc {
            response_intents.retain(|i| !matches!(i, CpdlcResponseIntent::Standby));
        }

        let needs_atc_response =
            !recent_closure && action_min.is_some() && !response_intents.is_empty();

        // Display text: use the original request text.
        let display_text = if standby_from_aircraft || standby_from_atc || recent_closure {
            action_source
                .and_then(|m| m.display_text.clone())
                .unwrap_or_else(|| {
                    req.display_text
                        .clone()
                        .unwrap_or_else(|| "Unknown request".into())
                })
        } else {
            req.display_text
                .clone()
                .unwrap_or_else(|| "Unknown request".into())
        };

        // Aircraft callsign: always resolve from action_source when available.
        let aircraft_callsign = if standby_from_aircraft || standby_from_atc || recent_closure {
            action_source
                .and_then(dialogue_callsign)
                .unwrap_or_else(|| cs.clone())
        } else {
            cs.clone()
        };

        // Card CSS class.
        let card_class = if standby_from_aircraft {
            "pending-request-item standby-aircraft"
        } else if standby_from_atc {
            "pending-request-item standby-atc"
        } else if recent_closure && closure_positive {
            "pending-request-item closed-positive"
        } else if recent_closure {
            "pending-request-item closed-negative"
        } else {
            "pending-request-item"
        };

        // Visual flag.
        let flag = if standby_from_aircraft || standby_from_atc {
            DialogueFlag::Standby(req.timestamp)
        } else if let Some(intent) = closure_intent {
            DialogueFlag::Closed {
                intent,
                positive: closure_positive,
            }
        } else if req.is_outgoing
            && !recent_closure
            && !is_closing_response_message(req)
            && !standby_from_atc
        {
            DialogueFlag::Received(req.timestamp)
        } else {
            DialogueFlag::None
        };

        // Sort priority.
        let priority = if recent_closure {
            2
        } else if needs_atc_response || !req.is_outgoing {
            0
        } else {
            1
        };

        // Only include response intents when action buttons should show.
        let final_intents = if needs_atc_response {
            response_intents
        } else {
            vec![]
        };

        out.push(PendingDialogue {
            aircraft_callsign,
            action_min,
            display_text,
            response_intents: final_intents,
            is_y_response,
            timestamp: req.timestamp,
            card_class,
            flag,
            priority,
        });
    }

    out.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| b.timestamp.cmp(&a.timestamp))
    });
    out
}

fn selected_group(key: &str) -> Option<&str> {
    if let Some(group) = key.strip_prefix("grp:") {
        return Some(group);
    }
    let cmd = key.strip_prefix("cmd:")?;
    Some(match cmd {
        "V_CLIMB_TO" | "V_DESCEND_TO" | "V_MAINTAIN" | "V_CLIMB_MAINTAIN" | "V_BLOCK_LEVEL"
        | "V_CROSS_LEVEL" => "vertical",
        "L_DIRECT_TO" | "L_FLY_HEADING" | "L_TURN_LEFT" | "L_TURN_RIGHT" | "L_ROUTE"
        | "L_HOLD" => "lateral",
        _ => "other",
    })
}

fn selected_command(key: &str) -> Option<&str> {
    key.strip_prefix("cmd:")
}

fn command_uplink_id(key: &str) -> Option<&'static str> {
    match key {
        "V_CLIMB_TO" => Some("UM20"),
        "V_DESCEND_TO" => Some("UM23"),
        "V_MAINTAIN" => Some("UM19"),
        "V_CLIMB_MAINTAIN" => Some("UM21"),
        "V_BLOCK_LEVEL" => Some("UM30"),
        "V_CROSS_LEVEL" => Some("UM46"),
        "L_DIRECT_TO" => Some("UM74"),
        "L_FLY_HEADING" => Some("UM190"),
        "L_TURN_LEFT" | "L_TURN_RIGHT" => Some("UM94"),
        "L_ROUTE" => Some("UM80"),
        "L_HOLD" => Some("UM92"),
        "O_SPEED" => Some("UM106"),
        "O_CONTACT" => Some("UM117"),
        "O_SQUAWK" => Some("UM123"),
        "O_REPORT" => Some("UM132"),
        "O_FREETEXT" => Some("UM169"),
        _ => None,
    }
}

fn arg_type_label(arg_type: openlink_models::ArgType) -> &'static str {
    use openlink_models::ArgType;
    match arg_type {
        ArgType::Level => "LEVEL",
        ArgType::Speed => "SPEED",
        ArgType::Time => "TIME",
        ArgType::Position => "POSITION",
        ArgType::Direction => "DIRECTION",
        ArgType::Degrees => "HEADING",
        ArgType::Distance => "DISTANCE",
        ArgType::RouteClearance => "ROUTE",
        ArgType::ProcedureName => "PROCEDURE",
        ArgType::UnitName => "UNIT",
        ArgType::FacilityDesignation => "FACILITY",
        ArgType::Frequency => "FREQUENCY",
        ArgType::Code => "CODE",
        ArgType::AtisCode => "ATIS",
        ArgType::ErrorInfo => "ERROR INFO",
        ArgType::FreeText => "TEXT",
        ArgType::VerticalRate => "VERTICAL RATE",
        ArgType::Altimeter => "ALTIMETER",
        ArgType::LegType => "LEG TYPE",
        ArgType::PositionReport => "POSITION REPORT",
        ArgType::RemainingFuel => "REMAINING FUEL",
        ArgType::PersonsOnBoard => "POB",
        ArgType::SpeedType => "SPEED TYPE",
        ArgType::DepartureClearance => "DEPARTURE CLR",
    }
}

fn arg_type_placeholder(arg_type: openlink_models::ArgType) -> &'static str {
    use openlink_models::ArgType;
    match arg_type {
        ArgType::Level => "Altitude / Niveau de vol (ex: FL350, 350, 12000)",
        ArgType::Speed => "Vitesse (ex: M.78, 280KT)",
        ArgType::Time => "Heure (ex: 1215)",
        ArgType::Position => "Position / Waypoint (ex: BOBIK, LFPG)",
        ArgType::Direction => "Direction (LEFT/RIGHT)",
        ArgType::Degrees => "Cap (ex: 270)",
        ArgType::Distance => "Distance (ex: 20NM)",
        ArgType::RouteClearance => "Route (ex: BOBIK DCT LGL UN872)",
        ArgType::ProcedureName => "Nom procedure",
        ArgType::UnitName => "Unite (ex: PARIS CONTROL)",
        ArgType::FacilityDesignation => "Designation unite",
        ArgType::Frequency => "Frequence (ex: 132.700)",
        ArgType::Code => "Code transpondeur (ex: 6421)",
        ArgType::AtisCode => "Code ATIS",
        ArgType::ErrorInfo => "Information d'erreur",
        ArgType::FreeText => "Texte libre",
        ArgType::VerticalRate => "Taux vertical",
        ArgType::Altimeter => "Reglage altimetrique",
        ArgType::LegType => "Type de segment",
        ArgType::PositionReport => "Compte-rendu de position",
        ArgType::RemainingFuel => "Carburant restant",
        ArgType::PersonsOnBoard => "Nombre de personnes a bord",
        ArgType::SpeedType => "Type de vitesse",
        ArgType::DepartureClearance => "Clairance depart",
    }
}

fn command_label(key: &str) -> String {
    if key == "L_TURN_LEFT" {
        return "TURN LEFT HEADING [degrees]".to_string();
    }
    if key == "L_TURN_RIGHT" {
        return "TURN RIGHT HEADING [degrees]".to_string();
    }
    let Some(id) = command_uplink_id(key) else {
        return "UNKNOWN".to_string();
    };
    if let Some(def) = find_definition(id) {
        def.template.to_string()
    } else {
        id.to_string()
    }
}

fn command_param_specs(key: &str) -> Vec<(String, String)> {
    // Convenience UX: left/right heading commands expose heading only.
    if matches!(key, "L_TURN_LEFT" | "L_TURN_RIGHT") {
        return vec![("HEADING".to_string(), "Cap (ex: 270)".to_string())];
    }

    let Some(id) = command_uplink_id(key) else {
        return vec![];
    };
    let Some(def) = find_definition(id) else {
        return vec![];
    };

    def.args
        .iter()
        .map(|arg| {
            (
                arg_type_label(*arg).to_string(),
                arg_type_placeholder(*arg).to_string(),
            )
        })
        .collect()
}

fn command_needs_param(key: &str) -> bool {
    !command_param_specs(key).is_empty()
}

fn arg_value(args: &[String], idx: usize) -> Option<String> {
    let value = args.get(idx)?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_level(raw: &str) -> Option<FlightLevel> {
    let s = raw.trim().to_uppercase();
    let digits = s.strip_prefix("FL").unwrap_or(&s);
    let value: u16 = digits.parse().ok()?;
    if s.starts_with("FL") {
        if value >= 1000 {
            return None;
        }
        Some(FlightLevel::new(value))
    } else {
        Some(FlightLevel::new(value))
    }
}

fn parse_argument(arg_type: openlink_models::ArgType, raw: &str) -> Option<CpdlcArgument> {
    use openlink_models::ArgType;
    let v = raw.trim();
    if v.is_empty() {
        return None;
    }
    Some(match arg_type {
        ArgType::Level => CpdlcArgument::Level(parse_level(v)?),
        ArgType::Speed => CpdlcArgument::Speed(v.to_string()),
        ArgType::Time => CpdlcArgument::Time(v.to_string()),
        ArgType::Position => CpdlcArgument::Position(v.to_uppercase()),
        ArgType::Direction => CpdlcArgument::Direction(v.to_uppercase()),
        ArgType::Degrees => CpdlcArgument::Degrees(v.parse::<u16>().ok()?),
        ArgType::Distance => CpdlcArgument::Distance(v.to_string()),
        ArgType::RouteClearance => CpdlcArgument::RouteClearance(v.to_string()),
        ArgType::ProcedureName => CpdlcArgument::ProcedureName(v.to_string()),
        ArgType::UnitName => CpdlcArgument::UnitName(v.to_string()),
        ArgType::FacilityDesignation => CpdlcArgument::FacilityDesignation(v.to_string()),
        ArgType::Frequency => CpdlcArgument::Frequency(v.to_string()),
        ArgType::Code => CpdlcArgument::Code(v.to_string()),
        ArgType::AtisCode => CpdlcArgument::AtisCode(v.to_string()),
        ArgType::ErrorInfo => CpdlcArgument::ErrorInfo(v.to_string()),
        ArgType::FreeText => CpdlcArgument::FreeText(v.to_string()),
        ArgType::VerticalRate => CpdlcArgument::VerticalRate(v.to_string()),
        ArgType::Altimeter => CpdlcArgument::Altimeter(v.to_string()),
        ArgType::LegType => CpdlcArgument::LegType(v.to_string()),
        ArgType::PositionReport => CpdlcArgument::PositionReport(v.to_string()),
        ArgType::RemainingFuel => CpdlcArgument::RemainingFuel(v.to_string()),
        ArgType::PersonsOnBoard => CpdlcArgument::PersonsOnBoard(v.to_string()),
        ArgType::SpeedType => CpdlcArgument::SpeedType(v.to_string()),
        ArgType::DepartureClearance => CpdlcArgument::DepartureClearance(v.to_string()),
    })
}

fn build_command_element(command: &str, args: &[String]) -> Option<MessageElement> {
    let id = command_uplink_id(command)?;
    let def = find_definition(id)?;

    // Convenience UX: direction is implicit in the selected command.
    if command == "L_TURN_LEFT" || command == "L_TURN_RIGHT" {
        let hdg_raw = arg_value(args, 0)?;
        let hdg = hdg_raw.parse::<u16>().ok()?;
        let dir = if command == "L_TURN_LEFT" {
            "LEFT".to_string()
        } else {
            "RIGHT".to_string()
        };
        return Some(MessageElement::new(
            id,
            vec![CpdlcArgument::Direction(dir), CpdlcArgument::Degrees(hdg)],
        ));
    }

    let mut parsed_args = Vec::with_capacity(def.args.len());
    for (idx, arg_type) in def.args.iter().enumerate() {
        let raw = arg_value(args, idx)?;
        parsed_args.push(parse_argument(*arg_type, &raw)?);
    }

    Some(MessageElement::new(id, parsed_args))
}

fn add_command_element(mut app_state: Signal<AppState>, tab_id: Uuid, command: &str) {
    let element = {
        let state = app_state.read();
        let Some(tab) = state.tab_by_id(tab_id) else { return; };
        if command_needs_param(command) {
            build_command_element(command, &tab.cmd_arg_inputs)
        } else {
            build_command_element(command, &[])
        }
    };

    let Some(element) = element else {
        return;
    };

    let mut state = app_state.write();
    if let Some(tab) = state.tab_mut_by_id(tab_id) {
        tab.compose_elements.push(element);
        tab.cmd_arg_inputs.clear();
    }
}

#[component]
pub fn AtcView(tab_id: Uuid, app_state: Signal<AppState>, nats_clients: Signal<NatsClients>) -> Element {
    let mut standby_tick = use_signal(|| 0_u64);
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            standby_tick += 1;
        }
    });

    let locale = use_locale();
    let tr = t(*locale.read());

    let _ = standby_tick();

    let state = app_state.read();
    let tab = match state.tab_by_id(tab_id) {
        Some(t) => t,
        None => return rsx! { p { "{tr.tab_not_found}" } },
    };

    let mut linked_flights: Vec<AtcLinkedFlight> = tab
        .atc_sessions
        .values()
        .filter_map(|session| {
            let callsign = session.aircraft.as_ref()?.to_string();
            let aircraft_address: AcarsEndpointAddress = session.aircraft_address.as_ref()?.clone();
            let phase = session
                .active_connection
                .as_ref()
                .map(|c| c.phase)
                .or_else(|| session.inactive_connection.as_ref().map(|c| c.phase))
                .unwrap_or(CpdlcConnectionPhase::Terminated);
            Some(AtcLinkedFlight {
                callsign: callsign.clone(),
                aircraft_callsign: callsign,
                aircraft_address,
                phase,
            })
        })
        .collect();
    linked_flights.sort_by(|a, b| a.callsign.cmp(&b.callsign));

    let selected_idx = tab.selected_flight_idx;
    let messages = tab.messages.clone();
    let callsign = tab.setup.callsign.clone();
    let selected_flight = selected_idx.and_then(|idx| linked_flights.get(idx).cloned());

    let pending_logons: Vec<AtcLinkedFlight> = linked_flights
        .iter()
        .filter(|f| f.phase == CpdlcConnectionPhase::LogonPending)
        .cloned()
        .collect();

    let pending_dialogues = build_pending_dialogues(&messages);

    let compose_elements = tab.compose_elements.clone();
    let compose_mrn = tab.compose_mrn;
    let compose_target_cs = tab.compose_target_callsign.clone();
    let selection_key = tab.cmd_search_query.clone();
    let group = selected_group(&selection_key);
    let command = selected_command(&selection_key);

    let compose_preview = if compose_elements.is_empty() {
        String::new()
    } else {
        compose_elements
            .iter()
            .map(render_element)
            .collect::<Vec<_>>()
            .join(" AND ")
    };

    // When responding to a pending request, lock target to the aircraft that
    // originated the request (compose_target_cs) regardless of left-panel selection.
    let compose_flight: Option<AtcLinkedFlight> = compose_target_cs
        .as_ref()
        .and_then(|cs| linked_flights.iter().find(|f| f.aircraft_callsign == *cs).cloned())
        .or_else(|| selected_flight.clone());

    let can_compose_on_selected = compose_flight
        .as_ref()
        .is_some_and(|f| f.phase == CpdlcConnectionPhase::Connected);

    drop(state);

    rsx! {
        div { class: "console-structured",
            div { class: "console-left-panel",
                div { class: "console-panel-header", "TRAFFIC SITUATION" }
                div { class: "traffic-grid-header traffic-grid-header-compact",
                    div { class: "grid-col", "CALLSIGN" }
                    div { class: "grid-col", "STATUS" }
                }
                div { class: "traffic-grid-body",
                    if linked_flights.is_empty() {
                        div { class: "traffic-row no-traffic", div { class: "grid-col-full", "NO FLIGHTS" } }
                    } else {
                        for (idx, flight) in linked_flights.iter().enumerate() {
                            {
                                let is_selected = selected_idx == Some(idx);
                                let row_class = if is_selected { "traffic-row selected" } else { "traffic-row" };
                                let status = phase_status_label(flight.phase);
                                rsx! {
                                    div {
                                        class: "{row_class} traffic-row-compact",
                                        onclick: move |_| {
                                            let mut state = app_state.write();
                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                tab.selected_flight_idx = if is_selected { None } else { Some(idx) };
                                            }
                                        },
                                        div { class: "grid-col acid", "{flight.aircraft_callsign}" }
                                        div { class: "grid-col status status-single", "{status}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            div { class: "console-center-panel",
                div { class: "console-panel-header", "COMMS MANAGEMENT UNIT" }

                div { class: "pending-requests-section",
                    div { class: "console-section-header", "PENDING REQUESTS QUEUE" }
                    div { class: "pending-requests-body",
                        if pending_logons.is_empty() && pending_dialogues.is_empty() {
                            div { class: "no-pending", "NO PENDING REQUESTS" }
                        } else {
                            for flight in pending_logons.iter() {
                                {
                                    let flight_clone = flight.clone();
                                    rsx! {
                                        div { class: "pending-logon-item",
                                            div { class: "logon-text", "{flight.aircraft_callsign} | LOGON REQUEST" }
                                            div { class: "request-actions",
                                                button {
                                                    class: "action-btn accept",
                                                    onclick: {
                                                        let callsign_clone = callsign.clone();
                                                        let f = flight_clone.clone();
                                                        move |_| {
                                                            handle_logon_response(app_state, tab_id, nats_clients, &callsign_clone, &f, true);
                                                        }
                                                    },
                                                    "ACCEPT"
                                                }
                                                button {
                                                    class: "action-btn reject",
                                                    onclick: {
                                                        let callsign_clone = callsign.clone();
                                                        let f = flight_clone.clone();
                                                        move |_| {
                                                            handle_logon_response(app_state, tab_id, nats_clients, &callsign_clone, &f, false);
                                                        }
                                                    },
                                                    "REJECT"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            for dlg in pending_dialogues.iter() {
                                {
                                    let priority_intents: Vec<CpdlcResponseIntent> = dlg.response_intents.iter().copied().filter(|i| is_priority_response_intent(*i)).collect();
                                    let more_intents: Vec<CpdlcResponseIntent> = dlg.response_intents.iter().copied().filter(|i| !is_priority_response_intent(*i)).collect();
                                    let has_actions = !dlg.response_intents.is_empty();
                                    let show_standby = dlg.response_intents.contains(&CpdlcResponseIntent::Standby);
                                    let flag_content = match &dlg.flag {
                                        DialogueFlag::Standby(ts) => Some(("standby-flag", format!("STANDBY {}", standby_elapsed_label(*ts)))),
                                        DialogueFlag::Closed { intent, positive } => {
                                            let cls = if *positive { "standby-flag closure-flag positive" } else { "standby-flag closure-flag negative" };
                                            Some((cls, intent.label().to_string()))
                                        }
                                        DialogueFlag::Received(ts) => Some(("standby-flag received-flag", format!("RECEIVED {}", standby_elapsed_label(*ts)))),
                                        DialogueFlag::None => None,
                                    };
                                    let cs = &dlg.aircraft_callsign;
                                    let text = &dlg.display_text;
                                    let card_class = dlg.card_class;
                                    rsx! {
                                        div { class: "{card_class}",
                                            div { class: "request-line",
                                                div { class: "request-text", "{cs} | {text}" }
                                                if let Some((flag_class, flag_label)) = &flag_content {
                                                    div { class: "{flag_class}", "{flag_label}" }
                                                }
                                            }
                                            if has_actions {
                                                div { class: "request-actions",
                                                    if dlg.is_y_response {
                                                        div { class: "action-split",
                                                            button {
                                                                class: "action-btn unable action-split-main",
                                                                onclick: {
                                                                    let callsign_clone = callsign.clone();
                                                                    let cs_clone = dlg.aircraft_callsign.clone();
                                                                    let target_min = dlg.action_min;
                                                                    move |_| {
                                                                        if let Some(min) = target_min {
                                                                            handle_quick_response(app_state, tab_id, nats_clients, &callsign_clone, &cs_clone, min, CpdlcResponseIntent::Unable);
                                                                        }
                                                                    }
                                                                },
                                                                "UNABLE"
                                                            }
                                                            button {
                                                                class: "action-btn-compose action-split-plus",
                                                                onclick: {
                                                                    let cs_clone = dlg.aircraft_callsign.clone();
                                                                    let target_min = dlg.action_min;
                                                                    move |_| {
                                                                        if let Some(min) = target_min {
                                                                            inject_response_in_composer(app_state, tab_id, CpdlcResponseIntent::Unable, min, &cs_clone);
                                                                        }
                                                                    }
                                                                },
                                                                "+"
                                                            }
                                                        }
                                                        if show_standby {
                                                            button {
                                                                class: "action-btn standby",
                                                                onclick: {
                                                                    let callsign_clone = callsign.clone();
                                                                    let cs_clone = dlg.aircraft_callsign.clone();
                                                                    let target_min = dlg.action_min;
                                                                    move |_| {
                                                                        if let Some(min) = target_min {
                                                                            handle_quick_response(app_state, tab_id, nats_clients, &callsign_clone, &cs_clone, min, CpdlcResponseIntent::Standby);
                                                                        }
                                                                    }
                                                                },
                                                                "STANDBY"
                                                            }
                                                        }
                                                        button {
                                                            class: "action-btn-compose action-btn-compose-main",
                                                            onclick: {
                                                                let cs_clone = dlg.aircraft_callsign.clone();
                                                                let target_min = dlg.action_min;
                                                                move |_| {
                                                                    if let Some(min) = target_min {
                                                                        open_response_composer(app_state, tab_id, min, &cs_clone);
                                                                    }
                                                                }
                                                            },
                                                            "COMPOSE"
                                                        }
                                                    } else {
                                                        for intent in priority_intents.iter() {
                                                            {
                                                                let intent_val = *intent;
                                                                let intent_label = intent_val.label().to_string();
                                                                let btn_class = action_btn_class(intent_val);
                                                                rsx! {
                                                                    div { class: "action-split",
                                                                        button {
                                                                            class: "{btn_class} action-split-main",
                                                                            onclick: {
                                                                                let callsign_clone = callsign.clone();
                                                                                let cs_clone = dlg.aircraft_callsign.clone();
                                                                                let target_min = dlg.action_min;
                                                                                move |_| {
                                                                                    if let Some(min) = target_min {
                                                                                        handle_quick_response(app_state, tab_id, nats_clients, &callsign_clone, &cs_clone, min, intent_val);
                                                                                    }
                                                                                }
                                                                            },
                                                                            "{intent_label}"
                                                                        }
                                                                        button {
                                                                            class: "action-btn-compose action-split-plus",
                                                                            onclick: {
                                                                                let cs_clone = dlg.aircraft_callsign.clone();
                                                                                let target_min = dlg.action_min;
                                                                                move |_| {
                                                                                    if let Some(min) = target_min {
                                                                                        inject_response_in_composer(app_state, tab_id, intent_val, min, &cs_clone);
                                                                                    }
                                                                                }
                                                                            },
                                                                            "+"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }

                                                        if !more_intents.is_empty() {
                                                            details {
                                                                class: "request-more-menu",
                                                                summary { class: "action-btn-compose", "MORE ▾" }
                                                                div { class: "request-more-list",
                                                                    for intent in more_intents.iter() {
                                                                        {
                                                                            let intent_val = *intent;
                                                                            let intent_label = intent_val.label().to_string();
                                                                            let btn_class = action_btn_class(intent_val);
                                                                            rsx! {
                                                                                div { class: "action-split",
                                                                                    button {
                                                                                        class: "{btn_class} action-split-main",
                                                                                        onclick: {
                                                                                            let callsign_clone = callsign.clone();
                                                                                            let cs_clone = dlg.aircraft_callsign.clone();
                                                                                            let target_min = dlg.action_min;
                                                                                            move |_| {
                                                                                                if let Some(min) = target_min {
                                                                                                    handle_quick_response(app_state, tab_id, nats_clients, &callsign_clone, &cs_clone, min, intent_val);
                                                                                                }
                                                                                            }
                                                                                        },
                                                                                        "{intent_label}"
                                                                                    }
                                                                                    button {
                                                                                        class: "action-btn-compose action-split-plus",
                                                                                        onclick: {
                                                                                            let cs_clone = dlg.aircraft_callsign.clone();
                                                                                            let target_min = dlg.action_min;
                                                                                            move |_| {
                                                                                                if let Some(min) = target_min {
                                                                                                    inject_response_in_composer(app_state, tab_id, intent_val, min, &cs_clone);
                                                                                                }
                                                                                            }
                                                                                        },
                                                                                        "+"
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }

                                                        button {
                                                            class: "action-btn-compose action-btn-compose-main",
                                                            onclick: {
                                                                let cs_clone = dlg.aircraft_callsign.clone();
                                                                let target_min = dlg.action_min;
                                                                move |_| {
                                                                    if let Some(min) = target_min {
                                                                        open_response_composer(app_state, tab_id, min, &cs_clone);
                                                                    }
                                                                }
                                                            },
                                                            "COMPOSE RESPONSE"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                div { class: "console-composer-section",
                    div { class: "console-section-header", "OUTGOING MESSAGE COMPOSER" }
                    if compose_flight.is_none() {
                        div { class: "composer-no-selection", "SELECT AIRCRAFT TO COMPOSE MESSAGES" }
                    } else if !can_compose_on_selected {
                        div { class: "composer-no-selection",
                            "SELECTED AIRCRAFT IS NOT CPDLC CONNECTED. MESSAGE COMPOSER IS DISABLED."
                        }
                    } else {
                        div { class: "composer-interface",
                            div { class: "message-preview",
                                div { class: "preview-header", "MESSAGE PREVIEW" }
                                div { class: "preview-content",
                                    if compose_preview.is_empty() {
                                        span { class: "preview-empty", "SELECT TEMPLATE AND ENTER PARAMETERS" }
                                    } else {
                                        "{compose_preview}"
                                    }
                                }
                                if let Some(mrn) = compose_mrn {
                                    div { class: "preview-header", "RESPONSE MODE (MRN={mrn})" }
                                }
                            }

                            if command.is_none() {
                                div { class: "step-header", "1. DOMAIN" }
                                div { class: "domain-grid",
                                    for (key, label) in [("vertical", "VERTICAL"), ("lateral", "LATERAL"), ("other", "OTHER")] {
                                        button {
                                            class: if group == Some(key) { "instruction-btn active" } else { "instruction-btn" },
                                            onclick: {
                                                let k = key.to_string();
                                                move |_| {
                                                    let mut state = app_state.write();
                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                        tab.cmd_search_query = format!("grp:{k}");
                                                        tab.contact_input.clear();
                                                        tab.cmd_arg_inputs.clear();
                                                    }
                                                }
                                            },
                                            "{label}"
                                        }
                                    }
                                }

                                if let Some(g) = group {
                                    div { class: "parameter-selection",
                                        div { class: "step-header", "2. MESSAGE TEMPLATE" }
                                        if g == "vertical" {
                                            div { class: "instruction-grid",
                                                for cmd in ["V_CLIMB_TO", "V_DESCEND_TO", "V_MAINTAIN", "V_CLIMB_MAINTAIN", "V_BLOCK_LEVEL"] {
                                                    button {
                                                        class: if command == Some(cmd) { "instruction-btn active" } else { "instruction-btn" },
                                                        onclick: {
                                                            let c = cmd.to_string();
                                                            move |_| {
                                                                let mut state = app_state.write();
                                                                if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                    tab.cmd_search_query = format!("cmd:{c}");
                                                                    tab.contact_input.clear();
                                                                    tab.cmd_arg_inputs = vec![String::new(); command_param_specs(&c).len()];
                                                                }
                                                            }
                                                        },
                                                        "{command_label(cmd)}"
                                                    }
                                                }
                                            }
                                            select {
                                                class: "instruction-dropdown",
                                                onchange: move |evt: Event<FormData>| {
                                                    let v = evt.value();
                                                    if v.is_empty() { return; }
                                                    let mut state = app_state.write();
                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                        tab.cmd_search_query = format!("cmd:{v}");
                                                        tab.contact_input.clear();
                                                        tab.cmd_arg_inputs = vec![String::new(); command_param_specs(&v).len()];
                                                    }
                                                },
                                                option { value: "", "More vertical templates..." }
                                                option { value: "V_CROSS_LEVEL", "CROSS AT LEVEL" }
                                            }
                                        } else if g == "lateral" {
                                            div { class: "instruction-grid",
                                                for cmd in ["L_DIRECT_TO", "L_FLY_HEADING", "L_TURN_LEFT", "L_TURN_RIGHT", "L_ROUTE"] {
                                                    button {
                                                        class: if command == Some(cmd) { "instruction-btn active" } else { "instruction-btn" },
                                                        onclick: {
                                                            let c = cmd.to_string();
                                                            move |_| {
                                                                let mut state = app_state.write();
                                                                if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                    tab.cmd_search_query = format!("cmd:{c}");
                                                                    tab.contact_input.clear();
                                                                    tab.cmd_arg_inputs = vec![String::new(); command_param_specs(&c).len()];
                                                                }
                                                            }
                                                        },
                                                        "{command_label(cmd)}"
                                                    }
                                                }
                                            }
                                            select {
                                                class: "instruction-dropdown",
                                                onchange: move |evt: Event<FormData>| {
                                                    let v = evt.value();
                                                    if v.is_empty() { return; }
                                                    let mut state = app_state.write();
                                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                        tab.cmd_search_query = format!("cmd:{v}");
                                                        tab.contact_input.clear();
                                                        tab.cmd_arg_inputs = vec![String::new(); command_param_specs(&v).len()];
                                                    }
                                                },
                                                option { value: "", "More lateral templates..." }
                                                option { value: "L_HOLD", "HOLD AT" }
                                            }
                                        } else {
                                            div { class: "instruction-grid",
                                                for cmd in ["O_SPEED", "O_CONTACT", "O_SQUAWK", "O_REPORT", "O_FREETEXT"] {
                                                    button {
                                                        class: if command == Some(cmd) { "instruction-btn active" } else { "instruction-btn" },
                                                        onclick: {
                                                            let c = cmd.to_string();
                                                            move |_| {
                                                                let mut state = app_state.write();
                                                                if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                    tab.cmd_search_query = format!("cmd:{c}");
                                                                    tab.contact_input.clear();
                                                                    tab.cmd_arg_inputs = vec![String::new(); command_param_specs(&c).len()];
                                                                }
                                                            }
                                                        },
                                                        "{command_label(cmd)}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if let Some(cmd) = command {
                                div { class: "parameter-selection",
                                    div { class: "step-header-row",
                                        div { class: "step-header", "3. PARAMETERS" }
                                        button {
                                            class: "step-close-btn",
                                            onclick: move |_| {
                                                let mut state = app_state.write();
                                                if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                    tab.cmd_search_query.clear();
                                                    tab.contact_input.clear();
                                                    tab.cmd_arg_inputs.clear();
                                                }
                                            },
                                            "CANCEL"
                                        }
                                    }
                                    div { class: "param-panel",
                                        div { class: "param-label", "{command_label(cmd)}" }
                                        if cmd == "L_TURN_LEFT" {
                                            div { class: "param-fixed-note", "DIRECTION PRESET: LEFT" }
                                        } else if cmd == "L_TURN_RIGHT" {
                                            div { class: "param-fixed-note", "DIRECTION PRESET: RIGHT" }
                                        }
                                        {
                                            let param_specs = command_param_specs(cmd);
                                            let grid_class = if param_specs.len() > 1 {
                                                "param-input-grid two-cols"
                                            } else {
                                                "param-input-grid"
                                            };
                                            rsx! {
                                                div { class: "{grid_class}",
                                                    for (idx, (arg_label, arg_placeholder)) in param_specs.iter().enumerate() {
                                                        {
                                                            let current_value = {
                                                                let state = app_state.read();
                                                                state
                                                                    .tab_by_id(tab_id)
                                                                    .and_then(|t| t.cmd_arg_inputs.get(idx).cloned())
                                                                    .unwrap_or_default()
                                                            };
                                                            rsx! {
                                                                div { class: "param-input-wrap",
                                                                    div { class: "param-input-label", "{arg_label}" }
                                                                    input {
                                                                        class: "param-input",
                                                                        placeholder: "{arg_placeholder}",
                                                                        value: "{current_value}",
                                                                        oninput: move |evt: Event<FormData>| {
                                                                            let mut state = app_state.write();
                                                                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                                                                if tab.cmd_arg_inputs.len() <= idx {
                                                                                    tab.cmd_arg_inputs.resize(idx + 1, String::new());
                                                                                }
                                                                                tab.cmd_arg_inputs[idx] = evt.value();
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        div { class: "param-action-split",
                                            button {
                                                class: "param-add-btn param-add-main",
                                                onclick: {
                                                    let c = cmd.to_string();
                                                    move |_| {
                                                        add_command_element(app_state, tab_id, &c);
                                                    }
                                                },
                                                "ADD TO MESSAGE"
                                            }
                                            button {
                                                class: "param-add-btn param-send-now",
                                                onclick: {
                                                    let c = cmd.to_string();
                                                    let flight = compose_flight.clone();
                                                    let callsign_clone = callsign.clone();
                                                    move |_| {
                                                        let Some(ref f) = flight else { return; };
                                                        let (elements, mrn) = {
                                                            let state = app_state.read();
                                                            let Some(tab) = state.tab_by_id(tab_id) else { return; };
                                                            let mut elements = tab.compose_elements.clone();
                                                            if let Some(element) = build_command_element(&c, &tab.cmd_arg_inputs) {
                                                                elements.push(element);
                                                            }
                                                            (elements, tab.compose_mrn)
                                                        };
                                                        if !elements.is_empty() {
                                                            send_composed_message(
                                                                app_state,
                                                                tab_id,
                                                                nats_clients,
                                                                &callsign_clone,
                                                                f,
                                                                elements,
                                                                mrn,
                                                            );
                                                        }
                                                    }
                                                },
                                                "SEND NOW"
                                            }
                                        }
                                    }
                                }
                            }

                            div { class: "composer-actions",
                                button {
                                    class: "clear-btn",
                                    onclick: move |_| {
                                        let mut state = app_state.write();
                                        if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                            tab.compose_elements.clear();
                                            tab.cmd_search_query.clear();
                                            tab.contact_input.clear();
                                            tab.cmd_arg_inputs.clear();
                                            tab.compose_mode = false;
                                            tab.compose_mrn = None;
                                            tab.compose_target_callsign = None;
                                            tab.atc_uplink_open = false;
                                        }
                                    },
                                    "CLEAR"
                                }
                                button {
                                    class: if compose_elements.is_empty() { "send-uplink disabled" } else { "send-uplink" },
                                    disabled: compose_elements.is_empty(),
                                    onclick: {
                                        let flight = compose_flight.clone();
                                        let callsign_clone = callsign.clone();
                                        let elements_clone = compose_elements.clone();
                                        let mrn = compose_mrn;
                                        move |_| {
                                            if let Some(ref f) = flight {
                                                if !elements_clone.is_empty() {
                                                    send_composed_message(
                                                        app_state,
                                                        tab_id,
                                                        nats_clients,
                                                        &callsign_clone,
                                                        f,
                                                        elements_clone.clone(),
                                                        mrn,
                                                    );
                                                }
                                            }
                                        }
                                    },
                                    "SEND CPDLC UPLINK"
                                }
                            }
                        }
                    }
                }
            }

            div { class: "console-right-panel",
                div { class: "console-panel-header", "MASTER LOG / HISTORY" }
                div { class: "master-log",
                    for msg in messages.iter().rev().take(50).filter(|m| !is_logical_ack(m)) {
                        {
                            let time_str = msg.timestamp.format("%H:%M:%S UTC").to_string();
                            let prefix_str = if msg.is_outgoing {
                                ">ATC>".to_string()
                            } else if let Some(ref from) = msg.from_callsign {
                                format!("<{from}>")
                            } else {
                                ">SYSTEM>".to_string()
                            };
                            let content = msg.display_text.as_deref().unwrap_or("UNKNOWN MESSAGE");
                            let has_lack = has_lack_for_outgoing(msg, &messages);
                            rsx! {
                                div { class: "log-entry",
                                    "[{time_str}] {prefix_str} {content}"
                                    if should_track_lack(msg) && !has_lack {
                                        span { class: "ack-pending-indicator", "  ⏳" }
                                    }
                                }
                            }
                        }
                    }
                    if messages.is_empty() {
                        div { class: "log-empty", "NO MESSAGES YET" }
                    }
                }
            }
        }
    }
}

fn handle_logon_response(
    mut app_state: Signal<AppState>,
    tab_id: Uuid,
    nats_clients: Signal<NatsClients>,
    callsign: &str,
    flight: &AtcLinkedFlight,
    accept: bool,
) {
    eprintln!(
        "[ATC SEND][LOGON] tab={} from={} to={} accept={}",
        tab_id,
        callsign,
        flight.aircraft_callsign,
        accept
    );

    let clients = nats_clients.read();
    if let Some(client) = clients.get(&tab_id) {
        let logon_resp = client.cpdlc_logon_response(
            callsign,
            &flight.aircraft_callsign,
            &flight.aircraft_address,
            accept,
        );
        let conn_req = if accept {
            Some(client.cpdlc_connection_request(
                callsign,
                &flight.aircraft_callsign,
                &flight.aircraft_address,
            ))
        } else {
            None
        };

        let client = client.clone();
        spawn(async move {
            if let Err(e) = client.send_to_server(logon_resp).await {
                eprintln!("[ATC SEND][LOGON] failed: {e}");
            }
            if let Some(req) = conn_req {
                if let Err(e) = client.send_to_server(req).await {
                    eprintln!("[ATC SEND][CONNECTION REQUEST] failed: {e}");
                }
            }
        });
    }

    let response_text = if accept { "LOGON ACCEPTED" } else { "LOGON REJECTED" };
    crate::push_outgoing_message_to(
        &mut app_state,
        tab_id,
        response_text,
        Some(&flight.aircraft_callsign),
    );
}

fn handle_quick_response(
    mut app_state: Signal<AppState>,
    tab_id: Uuid,
    nats_clients: Signal<NatsClients>,
    callsign: &str,
    aircraft_callsign: &str,
    min: u8,
    intent: CpdlcResponseIntent,
) {
    eprintln!(
        "[ATC SEND][QUICK] tab={} from={} to={} mrn={} response={}",
        tab_id,
        callsign,
        aircraft_callsign,
        min,
        intent.label()
    );

    let flight = {
        let state = app_state.read();
        let tab = match state.tab_by_id(tab_id) {
            Some(t) => t,
            None => return,
        };
        find_linked_flight(tab, aircraft_callsign)
    };

    let Some(flight) = flight else { return };
    let response_text = intent.label().to_string();

    let clients = nats_clients.read();
    if let Some(client) = clients.get(&tab_id) {
        let element_id = intent.uplink_id();
        let elements = vec![MessageElement::new(element_id, vec![])];
        let msg = client.cpdlc_station_application(
            callsign,
            &flight.aircraft_callsign,
            &flight.aircraft_address,
            elements,
            Some(min),
        );
        let response_min = match &msg {
            OpenLinkMessage::Acars(acars) => match &acars.message {
                AcarsMessage::CPDLC(cpdlc) => match &cpdlc.message {
                    CpdlcMessageType::Application(app) => Some(app.min),
                    _ => None,
                },
            },
            _ => None,
        };
        let client = client.clone();
        spawn(async move {
            if let Err(e) = client.send_to_server(msg).await {
                eprintln!("[ATC SEND][QUICK] failed: {e}");
            }
        });

        crate::push_outgoing_message_to_with_min_and_mrn(
            &mut app_state,
            tab_id,
            &response_text,
            Some(&flight.aircraft_callsign),
            response_min,
            Some(min),
        );
    }

    let mut state = app_state.write();
    if let Some(tab) = state.tab_mut_by_id(tab_id) {
        if let Some(m) = tab
            .messages
            .iter_mut()
            .find(|m| m.min == Some(min) && !m.is_outgoing && m.from_callsign.as_deref() == Some(aircraft_callsign))
        {
            if !matches!(intent, CpdlcResponseIntent::Standby) {
                m.responded = true;
            }
        }
    }
}

fn inject_response_in_composer(
    mut app_state: Signal<AppState>,
    tab_id: Uuid,
    intent: CpdlcResponseIntent,
    mrn: u8,
    aircraft_callsign: &str,
) {
    let mut state = app_state.write();
    if let Some(tab) = state.tab_mut_by_id(tab_id) {
        select_flight_by_callsign(tab, aircraft_callsign);
        tab.compose_mode = true;
        tab.compose_mrn = Some(mrn);
        tab.compose_target_callsign = Some(aircraft_callsign.to_string());
        tab.atc_uplink_open = true;
        tab.compose_elements
            .push(MessageElement::new(intent.uplink_id(), vec![]));
    }
}

fn open_response_composer(mut app_state: Signal<AppState>, tab_id: Uuid, mrn: u8, aircraft_callsign: &str) {
    let mut state = app_state.write();
    if let Some(tab) = state.tab_mut_by_id(tab_id) {
        select_flight_by_callsign(tab, aircraft_callsign);
        tab.compose_mode = true;
        tab.compose_mrn = Some(mrn);
        tab.compose_target_callsign = Some(aircraft_callsign.to_string());
        tab.atc_uplink_open = true;
    }
}

fn send_composed_message(
    mut app_state: Signal<AppState>,
    tab_id: Uuid,
    nats_clients: Signal<NatsClients>,
    callsign: &str,
    flight: &AtcLinkedFlight,
    elements: Vec<MessageElement>,
    mrn: Option<u8>,
) {
    if elements.is_empty() {
        return;
    }

    let clients = nats_clients.read();
    if let Some(client) = clients.get(&tab_id) {
        let ids = elements
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let rendered = elements
            .iter()
            .map(render_element)
            .collect::<Vec<_>>()
            .join(" / ");
        eprintln!(
            "[ATC SEND][COMPOSED] tab={} from={} to={} ids=[{}] text={}",
            tab_id,
            callsign,
            flight.aircraft_callsign,
            ids,
            rendered
        );

        let msg = client.cpdlc_station_application(
            callsign,
            &flight.aircraft_callsign,
            &flight.aircraft_address,
            elements.clone(),
            mrn,
        );
        let outgoing_min = match &msg {
            OpenLinkMessage::Acars(acars) => match &acars.message {
                AcarsMessage::CPDLC(cpdlc) => match &cpdlc.message {
                    CpdlcMessageType::Application(app) => Some(app.min),
                    _ => None,
                },
            },
            _ => None,
        };
        let client = client.clone();
        spawn(async move {
            if let Err(e) = client.send_to_server(msg).await {
                eprintln!("[ATC SEND][COMPOSED] failed: {e}");
            }
        });

        let text = elements
            .iter()
            .map(render_element)
            .collect::<Vec<_>>()
            .join(" / ");
        crate::push_outgoing_message_to_with_min_and_mrn(
            &mut app_state,
            tab_id,
            &text,
            Some(&flight.aircraft_callsign),
            outgoing_min,
            mrn,
        );
    }

    let mut state = app_state.write();
    if let Some(tab) = state.tab_mut_by_id(tab_id) {
        tab.atc_uplink_open = false;
        tab.compose_mode = false;
        tab.compose_mrn = None;
        tab.compose_target_callsign = None;
        tab.compose_elements.clear();
        tab.cmd_search_query.clear();
        tab.contact_input.clear();
    }
}
