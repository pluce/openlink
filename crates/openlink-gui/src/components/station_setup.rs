use dioxus::prelude::*;
use uuid::Uuid;

use crate::state::{AppState, NatsClients, SavedStation, StationType, TabPhase, SetupFields};
use crate::nats_client;
use crate::i18n::{use_locale, t};

#[component]
pub fn StationSetup(
    tab_id: Uuid,
    app_state: Signal<AppState>,
    nats_clients: Signal<NatsClients>,
) -> Element {
    let state = app_state.read();
    let tab = state.tab_by_id(tab_id);
    let setup = tab.map(|t| t.setup.clone()).unwrap_or_default();
    let saved_stations = state.saved_stations.clone();
    drop(state);

    let locale = use_locale();
    let tr = t(*locale.read());

    let mut error_msg: Signal<Option<String>> = use_signal(|| None);
    let mut connecting = use_signal(|| false);

    rsx! {
        div { class: "setup-container",
            h2 { class: "setup-title", "{tr.setup_title}" }

            // Saved stations list
            if !saved_stations.is_empty() {
                div { class: "saved-stations",
                    h3 { "{tr.recent_stations}" }
                    for station in saved_stations.iter() {
                        button {
                            class: "saved-station-btn",
                            onclick: {
                                let station = station.clone();
                                move |_| {
                                    let station = station.clone();
                                    let mut state = app_state.write();
                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                        tab.setup = SetupFields {
                                            station_type: station.station_type.clone(),
                                            network_id: station.network_id.clone(),
                                            network_address: station.network_address.clone(),
                                            callsign: station.callsign.clone(),
                                            acars_address: station.acars_address.clone(),
                                        };
                                    }
                                }
                            },
                            span { class: "saved-type",
                                {match station.station_type {
                                    StationType::Aircraft => "✈ ",
                                    StationType::Atc => "🗼 ",
                                }}
                            }
                            span { class: "saved-callsign", "{station.callsign}" }
                            span { class: "saved-network", " ({station.network_id}/{station.network_address})" }
                        }
                    }
                }
            }

            // Setup form
            div { class: "setup-form",
                div { class: "form-row",
                    label { "{tr.station_type}" }
                    div { class: "radio-group",
                        label {
                            input {
                                r#type: "radio",
                                name: "station_type_{tab_id}",
                                checked: matches!(setup.station_type, StationType::Aircraft),
                                oninput: move |_| {
                                    let mut state = app_state.write();
                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                        tab.setup.station_type = StationType::Aircraft;
                                    }
                                },
                            }
                            "{tr.aircraft_dcdu}"
                        }
                        label {
                            input {
                                r#type: "radio",
                                name: "station_type_{tab_id}",
                                checked: matches!(setup.station_type, StationType::Atc),
                                oninput: move |_| {
                                    let mut state = app_state.write();
                                    if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                        tab.setup.station_type = StationType::Atc;
                                    }
                                },
                            }
                            "{tr.atc}"
                        }
                    }
                }

                div { class: "form-row",
                    label { "{tr.network}" }
                    input {
                        r#type: "text",
                        value: "{setup.network_id}",
                        placeholder: "demonetwork",
                        oninput: move |evt: Event<FormData>| {
                            let mut state = app_state.write();
                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                tab.setup.network_id = evt.value();
                            }
                        },
                    }
                }

                div { class: "form-row",
                    label { "{tr.network_address_cid}" }
                    input {
                        r#type: "text",
                        value: "{setup.network_address}",
                        placeholder: "765283",
                        oninput: move |evt: Event<FormData>| {
                            let mut state = app_state.write();
                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                tab.setup.network_address = evt.value();
                            }
                        },
                    }
                }

                div { class: "form-row",
                    label { "{tr.callsign}" }
                    input {
                        r#type: "text",
                        value: "{setup.callsign}",
                        placeholder: "AFR123 / LFPG",
                        oninput: move |evt: Event<FormData>| {
                            let mut state = app_state.write();
                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                tab.setup.callsign = evt.value().to_uppercase();
                            }
                        },
                    }
                }

                div { class: "form-row",
                    label { "{tr.acars_address}" }
                    input {
                        r#type: "text",
                        value: "{setup.acars_address}",
                        placeholder: "39401A",
                        oninput: move |evt: Event<FormData>| {
                            let mut state = app_state.write();
                            if let Some(tab) = state.tab_mut_by_id(tab_id) {
                                tab.setup.acars_address = evt.value().to_uppercase();
                            }
                        },
                    }
                }

                if let Some(ref err) = *error_msg.read() {
                    div { class: "error-banner", "{err}" }
                }

                button {
                    class: "connect-btn",
                    disabled: *connecting.read(),
                    onclick: move |_| {
                        let state = app_state.read();
                        let tab = state.tab_by_id(tab_id).cloned();
                        drop(state);

                        if let Some(tab) = tab {
                            let setup = tab.setup.clone();
                            if setup.network_address.is_empty()
                                || setup.callsign.is_empty()
                                || setup.acars_address.is_empty()
                            {
                                error_msg.set(Some(tr.all_fields_required.to_string()));
                                return;
                            }

                            connecting.set(true);
                            error_msg.set(None);

                            let status_err_label = tr.status_send_error;
                            let conn_err_label = tr.connection_failed;
                            spawn(async move {
                                match nats_client::connect_nats(&setup.network_id, &setup.network_address).await {
                                    Ok(client) => {
                                        // Send online status
                                        if let Err(e) = nats_client::send_online_status(
                                            &client,
                                            &setup.network_id,
                                            &setup.network_address,
                                            &setup.callsign,
                                            &setup.acars_address,
                                        ).await {
                                            error_msg.set(Some(format!("{status_err_label}: {e}")));
                                            connecting.set(false);
                                            return;
                                        }

                                        // Save station to presets
                                        let saved = SavedStation {
                                            station_type: setup.station_type.clone(),
                                            network_id: setup.network_id.clone(),
                                            network_address: setup.network_address.clone(),
                                            callsign: setup.callsign.clone(),
                                            acars_address: setup.acars_address.clone(),
                                        };
                                        {
                                            let mut state = app_state.write();
                                            state.save_station(saved);
                                        }

                                        // Store client in shared map
                                        nats_clients.write().insert(tab_id, client.clone());

                                        // Transition tab to Connected, mark listener as not yet started
                                        let station_type = setup.station_type.clone();
                                        {
                                            let mut state = app_state.write();
                                            if let Some(t) = state.tab_mut_by_id(tab_id) {
                                                t.label = setup.callsign.clone();
                                                t.phase = TabPhase::Connected(station_type);
                                                t.nats_task_active = false; // App will pick this up and start the listener
                                            }
                                        }

                                        connecting.set(false);
                                    },
                                    Err(e) => {
                                        error_msg.set(Some(format!("{conn_err_label}: {e}")));
                                        connecting.set(false);
                                    }
                                }
                            });
                        }
                    },
                    {
                        let btn_label = if *connecting.read() { tr.connecting_label } else { tr.connect_label };
                        rsx! { "{btn_label}" }
                    }
                }
            }
        }
    }
}
