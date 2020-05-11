#![allow(unused_variables)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

#[cfg(test)]
#[macro_use]
extern crate speculate;

use mosquitto_client::{Mosquitto};
use std::io::{Error, ErrorKind};
use std::time::Duration;
use std::thread;
use clap::{App, load_yaml};
use env_logger::Env;
use log::{warn,info};
use chrono::{Local};

#[path = "../config.rs"]
pub mod config;
#[path = "../helper.rs"]
#[macro_use]
pub mod helper;
#[path = "../zone.rs"]
pub mod zone;
#[path = "../repository.rs"]
pub mod repository;
#[path = "../deciders.rs"]
pub mod deciders;
#[path = "../state_retriever.rs"]
pub mod state_retriever;
#[path = "../schema.rs"]
pub mod schema;

use crate::config::{load_config, has_config_changed, Settings};
use crate::helper::{print_info, send_to_zone};
use crate::deciders::{ZoneStateDecider, TemperatureStateDecider, HeaterDecider};
use crate::state_retriever::{StateRetriever, PinChanges};
use crate::repository::{PinStateRepository};
use arduino_mqtt_pin::pin::{PinOperation};
use std::sync::{Arc};
use diesel::{SqliteConnection, Connection};

embed_migrations!("migrations");


fn main() -> Result<(), Error>
{
    let yaml = load_yaml!("../cli.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let verbosity: u8 = matches.occurrences_of("verbose") as u8;
    let config_path = matches.value_of("config").unwrap_or("config.conf");
    let db_path = matches.value_of("db").unwrap_or("pins.sqlite3");

    env_logger::from_env(Env::default().default_filter_or(match verbosity { 1 => "debug", 2 => "trace", _ => "info"})).init();

    let connection = SqliteConnection::establish(db_path)
        .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Unable to connect to db: {:?}", e)))?;
    embedded_migrations::run(&connection)
        .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Unable to run migrations: {:?}", e)))?;

    info!("Using config path: {}", config_path);

    let (conf_temp, mut control_nodes) = load_config(config_path, verbosity)?;
    let config = Settings::new(conf_temp);

    let repository = Arc::new(PinStateRepository::new(&connection));
    let temperature_decider = TemperatureStateDecider::new(&config);
    let zone_decider = ZoneStateDecider::new(&temperature_decider, &config);
    let heater_decider = HeaterDecider::new(&repository, &config);
    let state_retriever = StateRetriever::new(&repository, &heater_decider, &zone_decider, &config);

    let client = Mosquitto::new(&format!("{}-main", config.name()));
    client.connect(&config.host(), 1883)
        .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Unable to connect to host: {} {}", config.host(), e)))?;

    /*
     * receive remote on :
     * prefix/nodes/some-node-id/analog/set/3 1
     * receive local on:
     * prefix/master/analog/timeout/3
     */
    let remote_set = format!("{}/nodes/+/current/#", config.name());
    let local_set = format!("{}/master/#", config.name());

    let remote_channel = client.subscribe(&remote_set, 0)
        .map(|a| { info!("Listening to: {}", remote_set); a })
        .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Unable to subscribe: {} {:?}", remote_set, e)))?;
    let local_channel = client.subscribe(&local_set, 0)
        .map(|a| { info!("Listening to: {}", local_set); a })
        .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Unable to subscribe: {} {:?}", local_set, e)))?;


    let mut m = client.callbacks(());
    let mrepository = Arc::clone(&repository);
    m.on_message(move |_,msg| {

        match PinOperation::from_message(&msg) {
            Ok(o) => mrepository.save_state(&o),
            Err(e) => {
                warn!("Failed to parse message {:?}", msg);
                warn!("{}", e);
            }
        }
    });

    loop {
        if has_config_changed(config_path, config.version()) {
            let (new_config, nodes) = load_config(config_path, verbosity)?;
            control_nodes = nodes;
            config.replace(new_config);
        }

        let controls: PinChanges = state_retriever.get_pins_expected_to_change(&control_nodes, &Local::now());
        if controls.len() > 0 {
            info!("States expected to change: {}", controls.iter().map(|(n, m)| m.len()).sum::<usize>());
        }

        for (control_name, pins) in &controls {
            for (pin, value) in pins {
                send_to_zone(&client, *pin, value.as_u16(), &config.name(), control_name);
            }
        }

        print_info(&repository, &control_nodes);

        for i in 0..20 {
            let conn_result = client.do_loop(-1)
                .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Mqtt error {}", e)));
            if !conn_result.is_ok() {
                client.reconnect()
                    .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Mqtt can not reconnect {}", e)))?;
            }
            thread::sleep(Duration::from_millis(500));
        }
    }
}
