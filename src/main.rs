#![allow(unused_variables)]

#[macro_use]
extern crate json;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;

use std::collections::{HashMap};
use std::fs::File;
use std::io::Read;
use yaml_rust::{YamlLoader};
use mosquitto_client::{Mosquitto};
use std::io::{Error, ErrorKind};
use std::time::Duration;
use std::thread;
use std::cell::RefCell;
use clap::{App};
use env_logger::Env;

use log::{debug, warn, error};

pub mod config;
pub mod helper;
pub mod zone;

use crate::config::{Config, States};
use crate::helper::{create_nodes, print_info, apply_heating};
use arduino_mqtt_pin::pin::{PinOperation, PinCollection};

fn main() -> Result<(), Error>
{

    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let config = matches.value_of("config").unwrap_or("config.conf");
    let verbosity = matches.occurrences_of("verbose");

    env_logger::from_env(Env::default().default_filter_or(match verbosity { 1 => "debug", 2 => "trace", _ => "info"})).init();

    info!("Using config: {}", config);

    let mut yaml_file = File::open(config)?;
    let mut contents = String::new();
    yaml_file.read_to_string(&mut contents)?;

    println!("Config loaded: {} Verbosity: {}", config, verbosity);

    let yaml_config = YamlLoader::load_from_str(&contents)
        .map_err(|err| error!("{:?}", err))
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Unable to parse yaml file"))?;

    let config = Config::from_yaml(&yaml_config[0])
        .map_err(|s| { error!("{}", s); s })
        .map_err(|err| Error::new(ErrorKind::InvalidData, "Unable to parse config section"))?;

    let control_nodes = create_nodes(&yaml_config[0])
        .map_err(|s| { println!("{}", s); s })
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Unable to create control configuration"))?;

    let state_ref = RefCell::new(States::new());

    let client = Mosquitto::new(&config.name);
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

        //state_ref.borrow_mut().push(op);

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

    loop {
        let count = apply_heating(&client, &control_nodes, &state_ref.borrow(), &config);
        if count > 0 {
            info!("States expected to change: {}", count);
        }

        print_info(&state_ref.borrow());

        client.do_loop(-1)
            .map_err(|_| Error::new(ErrorKind::NotConnected, format!("Mqtt disconnected")))?;

        thread::sleep(Duration::from_millis(2000));
    }
}
