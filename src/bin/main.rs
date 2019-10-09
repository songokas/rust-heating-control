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

    let states = RefCell::new(States::new());

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

        if states.borrow().contains_key(&op.node) {
            let result = states.borrow_mut().get_mut(&op.node).map(|hmap| {
                hmap.get_mut(&op.pin_state.pin).map(|col| {
                    col.push(&op.pin_state);
                }).unwrap_or_else(|| {
                    let mut arr = PinCollection::new();
                    arr.push(&op.pin_state.clone());
                    hmap.insert(op.pin_state.pin, arr);
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
            states.borrow_mut().insert(op.node, col);
        }
    });

    loop {
        let count = apply_heating(&client, &control_nodes, &states.borrow(), &config);
        if count > 0 {
            info!("States expected to change: {}", count);
        }

        print_info(&states.borrow());

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
