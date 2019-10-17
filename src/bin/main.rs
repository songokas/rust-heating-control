#![allow(unused_variables)]

#[macro_use]
extern crate json;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
#[macro_use]
extern crate derive_new;

use mosquitto_client::{Mosquitto};
use std::io::{Error, ErrorKind};
use std::time::Duration;
use std::thread;
use std::cell::RefCell;
use clap::{App};
use env_logger::Env;
use log::{debug, warn};

#[path = "../config.rs"]
pub mod config;
#[path = "../helper.rs"]
pub mod helper;
#[path = "../zone.rs"]
pub mod zone;

use crate::config::{load_config, create_nodes};
use crate::helper::{print_info, send_to_zone};
use crate::deciders::{ZoneDecider, TemperatureDecider, HeaterDecider};
use crate::state_retriever::{StateRetriever};
use crate::repository::{States, PinStateRepository};
use arduino_mqtt_pin::pin::{PinOperation};

fn main() -> Result<(), Error>
{

    let yaml = load_yaml!("../cli.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let config_path = matches.value_of("config").unwrap_or("config.conf");
    let verbosity: u8 = matches.occurrences_of("verbose") as u8;

    env_logger::from_env(Env::default().default_filter_or(match verbosity { 1 => "debug", 2 => "trace", _ => "info"})).init();

    info!("Using config path: {}", config_path);

    let (config, control_nodes) = load_config(config_path, verbosity)?;

    //let states = RefCell::new(States::new());

    let repository = RefCell::new(PinStateRepository::new(States::new()));
    let temperature_decider = TemperatureDecider::new(&config);
    let zone_decider = ZoneDecider::new(&temperature_decider);
    let heater_decider = HeaterDecider::new(&config);
    let state_retriever = StateRetriever::new(&repository, &heater_decider, &zone_decider, &config);

    let client = Mosquitto::new(&format!("{}-main", config.name));
    client.connect(&config.host, 1883)
        .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Unable to connect to host: {} {}", config.host, e)))?;


    /*
     * receive remote on :
     * prefix/nodes/some-node-id/analog/set/3 1
     * receive local on:
     * prefix/master/analog/timeout/3
     */
    let remote_set = format!("{}/nodes/+/current/#", config.name);
    let local_set = format!("{}/master/#", config.name);

    let remote_channel = client.subscribe(&remote_set, 0)
        .map(|a| { info!("Listening to: {}", remote_set); a })
        .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Unable to subscribe: {} {:?}", remote_set, e)))?;
    let local_channel = client.subscribe(&local_set, 0)
        .map(|a| { info!("Listening to: {}", local_set); a })
        .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Unable to subscribe: {} {:?}", local_set, e)))?;


    let mut m = client.callbacks(());
    m.on_message(|_,msg| {

        //debug!("Message received: {:?} {}", msg, msg.text());

        let op = PinOperation::from_message(&msg);
        if !op.is_ok() {
            warn!("Failed to parse message {:?}", msg);
            warn!("{}", op.err().unwrap_or("Failed to see error"));
            return;
        }
        let op = op.unwrap();

        repository.borrow_mut().save_state(op);

        //add_state(&mut states.borrow_mut(), &op);

    });

    loop {
        let controls: PinChanges = state_retriever.get_pins_expected_to_change(Local::now());
        if pins.len() > 0 {
            info!("States expected to change: {}", pins.len());
        }

        for (control_name, pins) in controls {
            for (pin, value) in pins {
                send_value(client, pin, value, control_name);
            }
        }

        print_info(&repository.borrow());

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
