use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum Locale {
    Fr,
    En,
}

#[allow(dead_code)]
impl Locale {
    pub fn label(&self) -> &'static str {
        match self {
            Locale::Fr => "FR",
            Locale::En => "EN",
        }
    }

    pub fn next(&self) -> Locale {
        match self {
            Locale::Fr => Locale::En,
            Locale::En => Locale::Fr,
        }
    }
}

/// Get the current locale from Dioxus context.
/// Must be called inside a component where `provide_context` was used.
pub fn use_locale() -> Signal<Locale> {
    use_context::<Signal<Locale>>()
}

/// All translatable strings in the application.
/// Call `t(locale)` to get the struct for a given locale.
#[allow(dead_code)]
pub struct T {
    // ── General ──
    pub new_tab: &'static str,
    pub create_tab_prompt: &'static str,
    pub tab_not_found: &'static str,

    // ── Station Setup ──
    pub setup_title: &'static str,
    pub recent_stations: &'static str,
    pub station_type: &'static str,
    pub aircraft_dcdu: &'static str,
    pub atc: &'static str,
    pub network: &'static str,
    pub network_address_cid: &'static str,
    pub callsign: &'static str,
    pub acars_address: &'static str,
    pub all_fields_required: &'static str,
    pub status_send_error: &'static str,
    pub connection_failed: &'static str,
    pub connecting_label: &'static str,
    pub connect_label: &'static str,

    // ── DCDU ──
    pub ground_station: &'static str,
    pub icao_placeholder: &'static str,
    pub cancel: &'static str,
    pub commands: &'static str,
    pub pilot_downlink: &'static str,
    pub no_commands_available: &'static str,
    pub received_messages: &'static str,

    // ── ATC ──
    pub flights: &'static str,
    pub no_flights_connected: &'static str,
    pub messages_for: &'static str,
    pub actions: &'static str,
    pub accept_logon: &'static str,
    pub reject: &'static str,
    pub flight_connected: &'static str,
    pub select_flight: &'static str,
    pub conn_management: &'static str,
    pub atc_uplink: &'static str,
    pub contact_station: &'static str,
    pub transfer_to: &'static str,
    pub target_station_placeholder: &'static str,
    pub end_service: &'static str,

    // ── Shared ──
    pub no_messages: &'static str,
}

pub fn t(locale: Locale) -> T {
    match locale {
        Locale::Fr => T {
            // General
            new_tab: "Nouveau",
            create_tab_prompt: "Créez un nouvel onglet pour commencer.",
            tab_not_found: "Onglet introuvable",

            // Station Setup
            setup_title: "Configuration de la station",
            recent_stations: "Stations récentes",
            station_type: "Type de station",
            aircraft_dcdu: " ✈ Avion (DCDU)",
            atc: " 🗼 ATC",
            network: "Réseau",
            network_address_cid: "Adresse réseau (CID)",
            callsign: "Callsign",
            acars_address: "Adresse ACARS",
            all_fields_required: "Tous les champs sont requis.",
            status_send_error: "Erreur envoi status",
            connection_failed: "Connexion échouée",
            connecting_label: "Connexion…",
            connect_label: "Connecter",

            // DCDU
            ground_station: "Station sol",
            icao_placeholder: "ICAO (ex: LFPG)",
            cancel: "ANNULER",
            commands: "Commandes",
            pilot_downlink: "PILOT DOWNLINK",
            no_commands_available: "(Aucune commande disponible pour le moment)",
            received_messages: "Messages reçus",

            // ATC
            flights: "Vols",
            no_flights_connected: "Aucun vol connecté",
            messages_for: "Messages",
            actions: "Actions",
            accept_logon: "✓ Accepter LOGON",
            reject: "✗ Rejeter",
            flight_connected: "Vol connecté",
            select_flight: "Sélectionnez un vol dans la liste",
            conn_management: "Gestion connexion",
            atc_uplink: "ATC UPLINK",
            contact_station: "Contacter station",
            transfer_to: "Transférer vers",
            target_station_placeholder: "Station (ex: LFPG)",
            end_service: "Fin de service",

            // Shared
            no_messages: "Aucun message",
        },
        Locale::En => T {
            // General
            new_tab: "New",
            create_tab_prompt: "Create a new tab to get started.",
            tab_not_found: "Tab not found",

            // Station Setup
            setup_title: "Station configuration",
            recent_stations: "Recent stations",
            station_type: "Station type",
            aircraft_dcdu: " ✈ Aircraft (DCDU)",
            atc: " 🗼 ATC",
            network: "Network",
            network_address_cid: "Network address (CID)",
            callsign: "Callsign",
            acars_address: "ACARS address",
            all_fields_required: "All fields are required.",
            status_send_error: "Status send error",
            connection_failed: "Connection failed",
            connecting_label: "Connecting…",
            connect_label: "Connect",

            // DCDU
            ground_station: "Ground station",
            icao_placeholder: "ICAO (e.g. EGLL)",
            cancel: "CANCEL",
            commands: "Commands",
            pilot_downlink: "PILOT DOWNLINK",
            no_commands_available: "(No commands available yet)",
            received_messages: "Received messages",

            // ATC
            flights: "Flights",
            no_flights_connected: "No flights connected",
            messages_for: "Messages",
            actions: "Actions",
            accept_logon: "✓ Accept LOGON",
            reject: "✗ Reject",
            flight_connected: "Flight connected",
            select_flight: "Select a flight from the list",
            conn_management: "Connection mgmt",
            atc_uplink: "ATC UPLINK",
            contact_station: "Contact station",
            transfer_to: "Transfer to",
            target_station_placeholder: "Station (e.g. EGLL)",
            end_service: "End service",

            // Shared
            no_messages: "No messages",
        },
    }
}
