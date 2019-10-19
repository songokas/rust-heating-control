// run simulate_nodes and main app to debug
#![allow(unused_variables)]

#[macro_use]
extern crate json;
#[macro_use]
extern crate log;
#[macro_use]
extern crate derive_new;

use std::collections::{HashMap};

use mosquitto_client::{Mosquitto};
use std::io::{Error, ErrorKind};
use std::time::Duration;
use std::thread;
use std::cell::RefCell;
use env_logger::Env;
use log::{debug, info, warn};

#[path = "../config.rs"]
pub mod config;
#[path = "../helper.rs"]
pub mod helper;
#[path = "../zone.rs"]
pub mod zone;
#[path = "../repository.rs"]
pub mod repository;

use crate::config::{load_config, ControlNodes};

fn send_temperature(client: &Mosquitto, namespace: &str, name: &str, pin: u8, value: f32) -> bool
{
    let topic = format!("{namespace}/nodes/{name}/current/temperature/{pin}", namespace=namespace, name=name, pin=pin);
    let result = client.publish(
        &topic,
        format!("{}", value).as_bytes(),
        1,
        true
    );

    if let Err(v) = result {
        warn!("Unable to send temperature to {} {:?}", name, v);
        return false;
    }

    //debug!("Sent temperature: {} {}", topic, value);
    true
}

fn send_pin(client: &Mosquitto, namespace: &str, name: &str, pin: u8, value: &u16) -> bool
{
    let topic = format!("{namespace}/nodes/{name}/current/analog/{pin}", namespace=namespace, name=name, pin=pin);
    let result = client.publish(
        &topic,
        format!("{}", value).as_bytes(),
        1,
        true
    );

    if let Err(v) = result {
        warn!("Unable to send data to {} {} {} {:?}", name, topic, value, v);
        return false;
    }

    //debug!("Sent pin: {} {}", topic, value);
    true
}

fn send_zones(client: &Mosquitto, config_name: &str, control_nodes: &ControlNodes, pin_states: &HashMap<u8, u16>, temperature: f32)
{
    for (node_name, control_node) in control_nodes {
        for (zone_name, zone) in &control_node.zones {
            send_temperature(client, &config_name, &zone_name, zone.sensor_pin, temperature);
            send_pin(client, config_name, &node_name, zone.control_pin, pin_states.get(&zone.control_pin).unwrap_or(&0));
        }

        if control_node.control_pin > 0 {
            send_pin(client, config_name, &node_name, control_node.control_pin, pin_states.get(&control_node.control_pin).unwrap_or(&0));
        }

    }
}

fn main() -> Result<(), Error>
{

    //env_logger::from_env(Env::default().default_filter_or("debug")).init();
    let (config, control_nodes) = load_config("src/config.yml", 0)?;
    env_logger::from_env(Env::default().default_filter_or("debug")).init();

    let client = Mosquitto::new("test1");//&format!("{}-simulate", config.name));
    client.connect(&config.host, 1883)
        .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Unable to connect to host: {} {:?}", config.host, e)))?;


    for (node_name, control_node) in &control_nodes {
        for (zone_name, zone) in &control_node.zones {
            let topic = format!("{main}/nodes/{name}/set/json", main=config.name, name=zone_name);
            client.subscribe(&topic, 0)
                .map(|a| { info!("Listening to: {}", topic); a })
                .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Unable to subscribe: {} {}", zone_name, e)))?;
        }

        let topic = format!("{main}/nodes/{name}/set/json", main=config.name, name=node_name);
        client.subscribe(&topic, 0)
            .map(|a| { info!("Listening to: {}", topic); a })
            .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Unable to subscribe: {} {:?}", node_name, e)))?;
    }

    let states = RefCell::new(HashMap::new());

    let mut m = client.callbacks(());
    m.on_message(|_,msg| {
        debug!("Received: {:?} {}", msg, msg.text());
        let j = json::parse(msg.text()).unwrap();
        states.borrow_mut().insert(j["pin"].as_u8().unwrap(), j["set"].as_u16().unwrap());
    });

    loop {

        send_zones(&client, &config.name, &control_nodes, &states.borrow(), 16.0);

        println!("{:?}", states.borrow());

        for i in 0..10 {
            let conn_result = client.do_loop(-1)
                .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Mqtt error {}", e)));
            if !conn_result.is_ok() {
                println!("{:?}", conn_result);
                client.reconnect()
                    .map_err(|e| Error::new(ErrorKind::NotConnected, format!("Mqtt can not reconnect {}", e)))?;
            }
            thread::sleep(Duration::from_millis(500));
        }
    }

}
