#![allow(unused_variables)]

#[macro_use]
extern crate json;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;

use std::collections::{HashMap};

use mosquitto_client::{Mosquitto};
use std::io::{Error, ErrorKind};
use std::time::Duration;
use std::thread;
use std::cell::RefCell;
use std::sync::{Arc};
use clap::{App};
use env_logger::Env;
use log::{debug, warn};

#[path = "../config.rs"]
pub mod config;
#[path = "../helper.rs"]
pub mod helper;
#[path = "../zone.rs"]
pub mod zone;

use crate::config::{States};
use crate::helper::{print_info, apply_heating, load_config};
use arduino_mqtt_pin::pin::{PinOperation, PinCollection};

fn main() -> Result<(), Error>
{

    let yaml = load_yaml!("../cli.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let config_path = matches.value_of("config").unwrap_or("config.conf");
    let verbosity: u8 = matches.occurrences_of("verbose") as u8;

    env_logger::from_env(Env::default().default_filter_or(match verbosity { 1 => "debug", 2 => "trace", _ => "info"})).init();

    info!("Using config path: {}", config_path);

    let (config, control_nodes) = load_config(config_path, verbosity)?;

    let state_ref = RefCell::new(States::new());

    let client = Arc::new(Mosquitto::new(&config.name));
    client.connect(&config.host, 1883)
        .map_err(|_| Error::new(ErrorKind::NotConnected, format!("Unable to connect to host: {}", config.host)))?;

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
        .map_err(|_| Error::new(ErrorKind::NotConnected, format!("Unable to subscribe: {}", remote_set)))?;
    let local_channel = client.subscribe(&local_set, 0)
        .map(|a| { info!("Listening to: {}", local_set); a })
        .map_err(|_| Error::new(ErrorKind::NotConnected, format!("Unable to subscribe: {}", local_set)))?;

    let mut mc = client.callbacks(());
    mc.on_message(|_,msg| {

        debug!("Message received: {:?}", msg);

        let op = PinOperation::from_message(&msg);
        if !op.is_ok() {
            warn!("Failed to parse message {:?}", msg);
            warn!("{}", op.err().unwrap_or("Failed to see error"));
            return;
        }
        let op = op.unwrap();

        if state_ref.borrow().contains_key(&op.node) {
            let result = state_ref.borrow_mut().get_mut(&op.node).map(|hmap| {
                hmap.get_mut(&op.pin_state.pin).map(|col| {
                    col.push(&op.pin_state);
                });
            });
            if !result.is_some() {
                warn!("Failed to add state");
            }
        } else {
            let mut col = HashMap::new();
            let mut arr = PinCollection::new();
            arr.push(&op.pin_state.clone());
            col.insert(op.pin_state.pin, arr);
            state_ref.borrow_mut().insert(op.node, col);
        }
    });

    let tclient = client.clone();
    let mosquitto_thread = thread::spawn(move || {
        tclient.loop_until_disconnect(-1);
        debug!("Client disconnected");
    });
    loop {
        let count = apply_heating(&client, &control_nodes, &state_ref.borrow(), &config);
        if count > 0 {
            info!("States expected to change: {}", count);
        }

        print_info(&state_ref.borrow());

        thread::sleep(Duration::from_millis(10000));
    }
}
