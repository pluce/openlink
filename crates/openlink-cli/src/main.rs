// mod tui;
// mod app_state;
// mod ui;

use clap::{Parser, Subcommand};
use openlink_models::{AcarsEndpointAddress, AcarsEndpointCallsign, AcarsEnvelope, AcarsMessage, AcarsRouting, AcarsRoutingEndpoint, ArgType, CpdlcArgument, CpdlcEnvelope, CpdlcMessageType, CpdlcMetaMessage, FlightLevel, ICAOAirportCode, MessageBuilder, MessageDirection, MessageElement, MetaMessage, NetworkAddress, NetworkId, OpenLinkEnvelope, OpenLinkMessage, SerializedMessagePayload, StationId, find_definition};
use openlink_sdk::OpenLinkClient;
use std::io;
// use crate::tui::{EventHandler, init, restore};
// use crate::app_state::{AppController};
// use crate::ui::atc::AtcApp;
// use crate::ui::pilot::PilotApp;

use clap::{Args};
use futures::StreamExt;

fn default_presence_heartbeat_seconds() -> u64 {
    std::env::var("CLI_PRESENCE_HEARTBEAT_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(25)
        .max(1)
}

async fn send_station_status(
    client: &OpenLinkClient,
    station_id: &StationId,
    status: openlink_models::StationStatus,
    endpoint: &AcarsRoutingEndpoint,
) {
    let meta_msg = OpenLinkMessage::Meta(MetaMessage::StationStatus(
        station_id.clone(),
        status,
        endpoint.clone(),
    ));

    if let Err(e) = client.send_to_server(meta_msg).await {
        eprintln!("Failed to publish station status: {e}");
    }
}

fn parse_cpdlc_argument(arg_type: ArgType, raw: &str) -> Result<CpdlcArgument, String> {
    match arg_type {
        ArgType::Level => raw
            .parse::<FlightLevel>()
            .map(CpdlcArgument::Level)
            .map_err(|e| format!("invalid level '{raw}': {e}")),
        ArgType::Degrees => raw
            .parse::<u16>()
            .map(CpdlcArgument::Degrees)
            .map_err(|_| format!("invalid degrees '{raw}': expected unsigned integer")),
        ArgType::Speed => Ok(CpdlcArgument::Speed(raw.to_string())),
        ArgType::Time => Ok(CpdlcArgument::Time(raw.to_string())),
        ArgType::Position => Ok(CpdlcArgument::Position(raw.to_string())),
        ArgType::Direction => Ok(CpdlcArgument::Direction(raw.to_string())),
        ArgType::Distance => Ok(CpdlcArgument::Distance(raw.to_string())),
        ArgType::RouteClearance => Ok(CpdlcArgument::RouteClearance(raw.to_string())),
        ArgType::ProcedureName => Ok(CpdlcArgument::ProcedureName(raw.to_string())),
        ArgType::UnitName => Ok(CpdlcArgument::UnitName(raw.to_string())),
        ArgType::FacilityDesignation => Ok(CpdlcArgument::FacilityDesignation(raw.to_string())),
        ArgType::Frequency => Ok(CpdlcArgument::Frequency(raw.to_string())),
        ArgType::Code => Ok(CpdlcArgument::Code(raw.to_string())),
        ArgType::AtisCode => Ok(CpdlcArgument::AtisCode(raw.to_string())),
        ArgType::ErrorInfo => Ok(CpdlcArgument::ErrorInfo(raw.to_string())),
        ArgType::FreeText => Ok(CpdlcArgument::FreeText(raw.to_string())),
        ArgType::VerticalRate => Ok(CpdlcArgument::VerticalRate(raw.to_string())),
        ArgType::Altimeter => Ok(CpdlcArgument::Altimeter(raw.to_string())),
        ArgType::LegType => Ok(CpdlcArgument::LegType(raw.to_string())),
        ArgType::PositionReport => Ok(CpdlcArgument::PositionReport(raw.to_string())),
        ArgType::RemainingFuel => Ok(CpdlcArgument::RemainingFuel(raw.to_string())),
        ArgType::PersonsOnBoard => Ok(CpdlcArgument::PersonsOnBoard(raw.to_string())),
        ArgType::SpeedType => Ok(CpdlcArgument::SpeedType(raw.to_string())),
        ArgType::DepartureClearance => Ok(CpdlcArgument::DepartureClearance(raw.to_string())),
    }
}
#[derive(Parser, Debug)]
#[command(name = "openlink-cli")]
#[command(about = "OpenLink System Demonstrator CLI")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Identifiant du réseau (ex: DEMONETWORK)
    #[arg(long)]
    pub network_id: NetworkId,

    /// Adresse sur le réseau (ex: 765283)
    #[arg(long)]
    pub network_address: NetworkAddress,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Commandes relatives aux messages ACARS
    Acars(AcarsArgs),
}

#[derive(Args, Debug)]
pub struct AcarsArgs {
    /// Callsign de l'aéronef ou de la station (ex: AFR123)
    #[arg(short, long)]
    pub callsign: AcarsEndpointCallsign,

    /// Adresse ICAO 24-bit ou identifiant (ex: 394A0B)
    #[arg(short, long)]
    pub address: AcarsEndpointAddress,

    #[command(subcommand)]
    pub command: AcarsCommands,
}

#[derive(Subcommand, Debug)]
pub enum AcarsCommands {
    /// Commandes CPDLC
    Cpdlc(CpdlcArgs),
    /// Signaler le statut en ligne
    Online {
        /// Maintenir le statut online via heartbeat jusqu'à Ctrl+C
        #[arg(long, default_value_t = false)]
        hold: bool,
        /// Intervalle heartbeat en secondes (si --hold)
        #[arg(long, default_value_t = default_presence_heartbeat_seconds())]
        heartbeat_seconds: u64,
    },
    /// Signaler explicitement le statut hors-ligne
    Offline,
}

#[derive(Args, Debug)]
pub struct CpdlcArgs {
    /// Agir en tant que pilote
    #[arg(long, group = "role")]
    pub pilot: bool,

    /// Agir en tant qu'ATC
    #[arg(long, group = "role")]
    pub atc: bool,

    #[arg(long)]
    pub aircraft_callsign: AcarsEndpointCallsign,

    #[arg(long)]
    pub aircraft_address: AcarsEndpointAddress,

    #[command(subcommand)]
    pub action: CpdlcAction,
}

#[derive(Subcommand, Debug)]
pub enum CpdlcAction {
    /// Écouter les messages CPDLC entrants
    Listen,
    /// Envoyer un message CPDLC
    Send {
        #[command(subcommand)]
        message: CpdlcMessageCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum CpdlcMessageCommand {
    // --- Messages Meta (Gestion de session) ---

    /// Demande de connexion (Logon)
    LogonRequest {
        /// Station au sol cible (ex: LFPG)
        #[arg(long)]
        station: AcarsEndpointCallsign,
        /// Origine du plan de vol (ex: LFPG)
        #[arg(long)]
        origin: ICAOAirportCode,
        /// Destination du plan de vol (ex: KJFK)
        #[arg(long)]
        destination: ICAOAirportCode,
    },
    
    /// Réponse à un Logon
    LogonResponse {
        /// Demande acceptée
        #[arg(long, action)]
        accepted: bool,
    },

    /// Demande de connexion directe
    ConnectionRequest,

    /// Réponse à une demande de connexion
    ConnectionResponse {
        #[arg(long, action)]
        accepted: bool,
        /// Station destinataire (requis côté pilote)
        #[arg(long)]
        station: Option<AcarsEndpointCallsign>,
    },

    /// Demande de contact avec une autre station / fréquence
    ContactRequest {
        #[arg(long)]
        station: AcarsEndpointCallsign,
    },

    /// Réponse à une demande de contact
    ContactResponse {
        #[arg(long, action)]
        accepted: bool,
        /// Station destinataire (requis côté pilote)
        #[arg(long)]
        station: Option<AcarsEndpointCallsign>,
    },

    /// Indique que le contact est terminé
    ContactComplete {
        /// Station destinataire
        #[arg(long)]
        station: AcarsEndpointCallsign,
    },

    /// Transférer le Logon vers une autre station
    LogonForward {
        #[arg(long)]
        flight: String,
        #[arg(long)]
        origin: ICAOAirportCode,
        #[arg(long)]
        destination: ICAOAirportCode,
        #[arg(long)]
        new_station: AcarsEndpointCallsign,
    },

    /// Définir la prochaine autorité de données (Next Data Authority)
    NextDataAuthority {
        /// Callsign de la prochaine autorité
        #[arg(long)]
        nda_callsign: AcarsEndpointCallsign,
        /// Adresse de la prochaine autorité (optionnel si déductible)
        #[arg(long)]
        nda_address: AcarsEndpointAddress,
    },

    /// Terminer le service CPDLC
    EndService,

    // --- Messages Application (ATC) ---

    /// (Uplink) Monter au niveau de vol
    ClimbTo {
        #[arg(long)]
        level: String,
    },

    /// (Downlink) Demande de changement de niveau
    RequestLevelChange {
        #[arg(long)]
        level: String,
        /// Station destinataire de la demande
        #[arg(long)]
        station: AcarsEndpointCallsign,
    },

    /// Generic UM/DM application message by ICAO ID (supports all registry messages)
    UmDm {
        /// Message ID like UM20, DM67, etc.
        #[arg(long)]
        id: String,
        /// Comma-separated ordered args matching the message definition
        #[arg(long, value_delimiter = ',')]
        args: Vec<String>,
        /// Optional MRN for response messages
        #[arg(long)]
        mrn: Option<u8>,
        /// Destination callsign override (defaults to aircraft for ATC and station for pilot)
        #[arg(long)]
        to: Option<AcarsEndpointCallsign>,
    },
}

fn cpdlc_message(aircraft_callsign: AcarsEndpointCallsign, aircraft_address: AcarsEndpointAddress, my_callsign: AcarsEndpointCallsign, destination: AcarsEndpointCallsign, message: CpdlcMessageType) -> OpenLinkMessage {
    return OpenLinkMessage::Acars(AcarsEnvelope {
        routing: AcarsRouting {
            aircraft: AcarsRoutingEndpoint::new(aircraft_callsign, aircraft_address),
        },
        message: AcarsMessage::CPDLC(CpdlcEnvelope {
            source: my_callsign.clone(),
            destination: destination.clone(),
            message: message,
        })
    })
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    
    let cli = Cli::parse();
    
    let network_id = cli.network_id.clone();
    let network_address = cli.network_address.clone();

    println!("Starting for {:?}:{:?}", network_id,network_address);

    println!("DEBUG: Acquired OIDC Token for {:?}: {}", cli.network_address, network_address.to_string());

    let client = OpenLinkClient::connect_with_authorization_code(
        &nats_url, 
        "http://localhost:3001", 
        &network_address.to_string(),
        &network_id,
    ).await.expect("Failed to connect");
    
    let cid = client.cid().to_string();
    println!("DEBUG: Connected. Resolved CID: '{}'", cid);

    match cli.command {
        Commands::Acars(acars_args) => {
            let callsign = acars_args.callsign;
            let address = acars_args.address;
            let acars_endpoint = AcarsRoutingEndpoint::new(callsign.clone(), address.clone());

            match acars_args.command {
                AcarsCommands::Cpdlc(cpdlc_args) => {
                    let CpdlcArgs { aircraft_callsign, aircraft_address, pilot: is_pilot, atc: is_atc, action} = cpdlc_args;
                    // Handle CPDLC commands
                    match action {
                        CpdlcAction::Listen { } => {
                            println!("Listening for CPDLC messages");
                            let station_id = StationId::new(network_address.to_string().as_str());
                            send_station_status(
                                &client,
                                &station_id,
                                openlink_models::StationStatus::Online,
                                &acars_endpoint,
                            )
                            .await;

                            let mut subscriber = client.subscribe_inbox().await.expect("Failed to subscribe to inbox");
                            let mut heartbeat =
                                tokio::time::interval(std::time::Duration::from_secs(default_presence_heartbeat_seconds()));

                            loop {
                                tokio::select! {
                                    _ = heartbeat.tick() => {
                                        send_station_status(
                                            &client,
                                            &station_id,
                                            openlink_models::StationStatus::Online,
                                            &acars_endpoint,
                                        )
                                        .await;
                                    }
                                    maybe_message = subscriber.next() => {
                                        let Some(message) = maybe_message else {
                                            break;
                                        };
                                        match serde_json::from_slice::<OpenLinkEnvelope>(&message.payload) {
                                            Ok(envelope) => {
                                                // Extract source and display text from CPDLC payload
                                                let (source, display) = match &envelope.payload {
                                                    OpenLinkMessage::Acars(acars_env) => {
                                                        match &acars_env.message {
                                                            AcarsMessage::CPDLC(cpdlc_env) => {
                                                                let serialized: SerializedMessagePayload = cpdlc_env.message.clone().into();
                                                                (cpdlc_env.source.to_string(), serialized.to_string())
                                                            }
                                                        }
                                                    }
                                                    _ => ("unknown".to_string(), format!("{:?}", envelope.payload)),
                                                };
                                                println!("[{}] {} → {}", envelope.timestamp.format("%H:%M:%S"), source, display);
                                            }
                                            Err(e) => {
                                                let raw = String::from_utf8_lossy(&message.payload);
                                                println!("⚠ Parse error: {e} — raw: {raw}");
                                            }
                                        }
                                    }
                                    _ = tokio::signal::ctrl_c() => {
                                        println!("Stopping listener...");
                                        break;
                                    }
                                }
                            }

                            send_station_status(
                                &client,
                                &station_id,
                                openlink_models::StationStatus::Offline,
                                &acars_endpoint,
                            )
                            .await;
                        },
                        CpdlcAction::Send { message } => {
                            println!("Sending CPDLC message: {:?}", message);

                            match (is_pilot, is_atc, message) {
                                (true, false, CpdlcMessageCommand::LogonRequest { station, origin, destination }) => {
                                    println!("Preparing Logon Request for station '{:?}', origin '{:?}', destination '{:?}'", station, origin, destination);
                                    println!("DEBUG: Publishing Logon Request for {:?}...", callsign);
                                    client.send_to_server(cpdlc_message(
                                        aircraft_callsign,
                                        aircraft_address,
                                        callsign,
                                        station.clone(),
                                        CpdlcMessageType::Meta(CpdlcMetaMessage::LogonRequest {
                                            station: station.clone(),
                                            flight_plan_origin: origin,
                                            flight_plan_destination: destination
                                        })
                                    )).await.expect("Failed to send logon request");
                                },
                                (false, true, CpdlcMessageCommand::LogonResponse { accepted }) => {
                                    println!("Preparing Logon Response for aircraft '{:?}' - accepted: {:?}", aircraft_callsign, accepted);
                                    println!("DEBUG: Publishing Logon Response for {:?}...", callsign);
                                    client.send_to_server(cpdlc_message(
                                        aircraft_callsign.clone(),
                                        aircraft_address,
                                        callsign,
                                        aircraft_callsign.clone(),
                                        CpdlcMessageType::Meta(CpdlcMetaMessage::LogonResponse { accepted })
                                    )).await.expect("Failed to send logon response");
                                },
                                (false, true, CpdlcMessageCommand::ConnectionRequest) => {
                                    println!("Preparing Connection Request for aircraft '{:?}'", aircraft_callsign);
                                    client.send_to_server(cpdlc_message(
                                        aircraft_callsign.clone(),
                                        aircraft_address,
                                        callsign,
                                        aircraft_callsign.clone(),
                                        CpdlcMessageType::Meta(CpdlcMetaMessage::ConnectionRequest)
                                    )).await.expect("Failed to send connection request");
                                },
                                (true, false, CpdlcMessageCommand::ConnectionResponse { accepted, station }) => {
                                    if let Some(station) = station {
                                        println!("Preparing Connection Response for station '{:?}' - accepted: {:?}", station, accepted);
                                        client.send_to_server(cpdlc_message(
                                            aircraft_callsign.clone(),
                                            aircraft_address,
                                            callsign,
                                            station,
                                            CpdlcMessageType::Meta(CpdlcMetaMessage::ConnectionResponse { accepted })
                                        )).await.expect("Failed to send connection response");
                                    } else {
                                        eprintln!("--station is required for pilot connection-response");
                                    }
                                },
                                (false, true, CpdlcMessageCommand::ContactRequest { station }) => {
                                    println!("Preparing Contact Request to aircraft '{:?}' toward station '{:?}'", aircraft_callsign, station);
                                    client.send_to_server(cpdlc_message(
                                        aircraft_callsign.clone(),
                                        aircraft_address,
                                        callsign,
                                        aircraft_callsign.clone(),
                                        CpdlcMessageType::Meta(CpdlcMetaMessage::ContactRequest { station })
                                    )).await.expect("Failed to send contact request");
                                },
                                (true, false, CpdlcMessageCommand::ContactResponse { accepted, station }) => {
                                    if let Some(station) = station {
                                        println!("Preparing Contact Response to station '{:?}' - accepted: {:?}", station, accepted);
                                        client.send_to_server(cpdlc_message(
                                            aircraft_callsign.clone(),
                                            aircraft_address,
                                            callsign,
                                            station,
                                            CpdlcMessageType::Meta(CpdlcMetaMessage::ContactResponse { accepted })
                                        )).await.expect("Failed to send contact response");
                                    } else {
                                        eprintln!("--station is required for pilot contact-response");
                                    }
                                },
                                (_, _, CpdlcMessageCommand::ContactComplete { station }) => {
                                    println!("Preparing Contact Complete to '{:?}'", station);
                                    client.send_to_server(cpdlc_message(
                                        aircraft_callsign.clone(),
                                        aircraft_address,
                                        callsign,
                                        station,
                                        CpdlcMessageType::Meta(CpdlcMetaMessage::ContactComplete)
                                    )).await.expect("Failed to send contact complete");
                                },
                                (false, true, CpdlcMessageCommand::LogonForward { flight, origin, destination, new_station }) => {
                                    println!("Preparing Logon Forward to '{:?}' for flight '{}'", new_station, flight);
                                    client.send_to_server(cpdlc_message(
                                        aircraft_callsign.clone(),
                                        aircraft_address,
                                        callsign,
                                        new_station.clone(),
                                        CpdlcMessageType::Meta(CpdlcMetaMessage::LogonForward {
                                            flight: AcarsEndpointCallsign::new(&flight),
                                            flight_plan_origin: origin,
                                            flight_plan_destination: destination,
                                            new_station,
                                        })
                                    )).await.expect("Failed to send logon forward");
                                },
                                (false, true, CpdlcMessageCommand::NextDataAuthority { nda_callsign, nda_address }) => {
                                    println!("Preparing Next Data Authority '{}' for aircraft '{:?}'", nda_callsign, aircraft_callsign);
                                    client.send_to_server(cpdlc_message(
                                        aircraft_callsign.clone(),
                                        aircraft_address,
                                        callsign,
                                        aircraft_callsign.clone(),
                                        CpdlcMessageType::Meta(CpdlcMetaMessage::NextDataAuthority {
                                            nda: AcarsRoutingEndpoint::new(nda_callsign, nda_address),
                                        })
                                    )).await.expect("Failed to send next data authority");
                                },
                                (false, true, CpdlcMessageCommand::EndService) => {
                                    println!("Preparing END SERVICE for aircraft '{:?}'", aircraft_callsign);
                                    client.send_to_server(cpdlc_message(
                                        aircraft_callsign.clone(),
                                        aircraft_address,
                                        callsign,
                                        aircraft_callsign.clone(),
                                        CpdlcMessageType::Meta(CpdlcMetaMessage::EndService)
                                    )).await.expect("Failed to send END SERVICE");
                                },
                                (false, true, CpdlcMessageCommand::ClimbTo { level }) => {
                                    if let Ok(level) = level.parse::<FlightLevel>() {
                                        let msg = MessageBuilder::cpdlc(
                                            aircraft_callsign.to_string(),
                                            aircraft_address.to_string(),
                                        )
                                        .from(callsign.to_string())
                                        .to(aircraft_callsign.to_string())
                                        .climb_to(level)
                                        .build();
                                        client.send_to_server(msg).await.expect("Failed to send CLIMB TO");
                                    } else {
                                        eprintln!("Invalid flight level: {level}");
                                    }
                                },
                                (true, false, CpdlcMessageCommand::RequestLevelChange { level, station }) => {
                                    if let Ok(level) = level.parse::<FlightLevel>() {
                                        let msg = MessageBuilder::cpdlc(
                                            aircraft_callsign.to_string(),
                                            aircraft_address.to_string(),
                                        )
                                        .from(callsign.to_string())
                                        .to(station.to_string())
                                        .request_level(level)
                                        .build();
                                        client.send_to_server(msg).await.expect("Failed to send REQUEST LEVEL CHANGE");
                                    } else {
                                        eprintln!("Invalid flight level: {level}");
                                    }
                                },
                                (_, _, CpdlcMessageCommand::UmDm { id, args, mrn, to }) => {
                                    let id_upper = id.trim().to_ascii_uppercase();
                                    let Some(def) = find_definition(&id_upper) else {
                                        eprintln!("Unknown CPDLC message ID: {id_upper}");
                                        return Ok(());
                                    };

                                    if args.len() != def.args.len() {
                                        eprintln!(
                                            "Argument count mismatch for {}: expected {}, got {}",
                                            id_upper,
                                            def.args.len(),
                                            args.len()
                                        );
                                        return Ok(());
                                    }

                                    if is_atc && def.direction == MessageDirection::Downlink {
                                        eprintln!("Warning: {} is a DM (downlink) message but role is --atc", id_upper);
                                    }
                                    if is_pilot && def.direction == MessageDirection::Uplink {
                                        eprintln!("Warning: {} is a UM (uplink) message but role is --pilot", id_upper);
                                    }

                                    let parsed_args: Result<Vec<CpdlcArgument>, String> = def
                                        .args
                                        .iter()
                                        .zip(args.iter())
                                        .map(|(expected, raw)| parse_cpdlc_argument(*expected, raw))
                                        .collect();

                                    let parsed_args = match parsed_args {
                                        Ok(v) => v,
                                        Err(e) => {
                                            eprintln!("Failed to parse arguments: {e}");
                                            return Ok(());
                                        }
                                    };

                                    let destination = match to {
                                        Some(dest) => dest.to_string(),
                                        None if is_atc => aircraft_callsign.to_string(),
                                        None if is_pilot => {
                                            eprintln!("--to is required for pilot when sending generic UM/DM messages");
                                            return Ok(());
                                        }
                                        None => {
                                            eprintln!("Specify one role: --pilot or --atc");
                                            return Ok(());
                                        }
                                    };

                                    let msg = MessageBuilder::cpdlc(
                                        aircraft_callsign.to_string(),
                                        aircraft_address.to_string(),
                                    )
                                    .from(callsign.to_string())
                                    .to(destination)
                                    .application_message_with_mrn(
                                        vec![MessageElement::new(id_upper, parsed_args)],
                                        mrn,
                                    )
                                    .build();
                                    client.send_to_server(msg).await.expect("Failed to send generic UM/DM message");
                                },
                                _ => {
                                    println!("Invalid role/message combination. Check --pilot/--atc and required arguments.");
                                }
                            }
                        },
                    }
                },
                AcarsCommands::Online { hold, heartbeat_seconds } => {
                    let station_id = StationId::new(network_address.to_string().as_str());
                    println!("DEBUG: Publishing Online Status for {:?}...", callsign);
                    send_station_status(
                        &client,
                        &station_id,
                        openlink_models::StationStatus::Online,
                        &acars_endpoint,
                    )
                    .await;

                    if hold {
                        println!(
                            "Holding online presence (heartbeat={}s). Press Ctrl+C to exit.",
                            heartbeat_seconds
                        );
                        let mut heartbeat =
                            tokio::time::interval(std::time::Duration::from_secs(heartbeat_seconds.max(1)));
                        loop {
                            tokio::select! {
                                _ = heartbeat.tick() => {
                                    send_station_status(
                                        &client,
                                        &station_id,
                                        openlink_models::StationStatus::Online,
                                        &acars_endpoint,
                                    )
                                    .await;
                                }
                                _ = tokio::signal::ctrl_c() => {
                                    println!("Stopping online presence...");
                                    break;
                                }
                            }
                        }

                        send_station_status(
                            &client,
                            &station_id,
                            openlink_models::StationStatus::Offline,
                            &acars_endpoint,
                        )
                        .await;
                    }
                }
                AcarsCommands::Offline => {
                    let station_id = StationId::new(network_address.to_string().as_str());
                    println!("DEBUG: Publishing Offline Status for {:?}...", callsign);
                    send_station_status(
                        &client,
                        &station_id,
                        openlink_models::StationStatus::Offline,
                        &acars_endpoint,
                    )
                    .await;
                }
            }
        }
    }

    Ok(())
}
