// mod tui;
// mod app_state;
// mod ui;

use clap::{Parser, Subcommand};
use openlink_models::{AcarsEndpointAddress, AcarsEndpointCallsign, AcarsEnvelope, AcarsMessage, AcarsRouting, AcarsRoutingEndpoint, CpdlcEnvelope, CpdlcMessageType, CpdlcMetaMessage, ICAOAirportCode, MetaMessage, NetworkAddress, NetworkId, OpenLinkEnvelope, OpenLinkMessage, SerializedMessagePayload, StationId};
use openlink_sdk::OpenLinkClient;
use std::io;
// use crate::tui::{EventHandler, init, restore};
// use crate::app_state::{AppController};
// use crate::ui::atc::AtcApp;
// use crate::ui::pilot::PilotApp;

use clap::{Args};
use futures::StreamExt;
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
    Online,
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
    },

    /// Indique que le contact est terminé
    ContactComplete,

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
                            let mut subscriber = client.subscribe_inbox().await.expect("Failed to subscribe to inbox");
                            while let Some(message) = subscriber.next().await {
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
                                _ => {
                                    println!("Other CPDLC message types are not implemented in this demo.");
                                }
                            }
                            // Note: In a real implementation, we would construct the appropriate OpenLinkEnvelope with the CPDLC message and publish it to NATS here.
                        },
                    }
                },
                AcarsCommands::Online => {
                    // Send a Meta Message to signal online status
                    let meta_msg = OpenLinkMessage::Meta(
                        MetaMessage::StationStatus(
                            StationId::new(network_address.to_string().as_str()),
                            openlink_models::StationStatus::Online,
                            acars_endpoint.clone()
                        )
                    );

                    println!("DEBUG: Publishing Online Status for {:?}...", callsign);
                    client.send_to_server(meta_msg).await.expect("Failed to publish status");
                }
            }
        }
    }

    Ok(())
}
