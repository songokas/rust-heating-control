use std::collections::{HashMap};
use yaml_rust::{Yaml, YamlLoader};
use chrono::{Local};
use mosquitto_client::{Mosquitto};
use log::{debug, warn, error};
use std::fs::File;
use std::io::{Read, Error, ErrorKind};

use arduino_mqtt_pin::pin::{Temperature, PinCollection, PinOperation};
use arduino_mqtt_pin::helper::{percent_to_analog, more_recent_date};

use crate::zone::{Zone};
use crate::config::{ControlNodes, ControlNode, States, Config};

pub fn send_to_zone(client: &Mosquitto, pin: u8, value: u16, namespace: &str, name: &str) -> bool
{
    let data = object!{
        "pin" => pin,
        "set" => value
    };

    let topic = format!("{namespace}/nodes/{name}/set/json", namespace=namespace, name=name);

    let result = client.publish(
        &topic,
        data.dump().as_bytes(),
        1,
        true
    );

    debug!("Message sent: {} {}", topic, data.dump());

    if let Err(v) = result {
        warn!("Unable to send data to {}", name);
        return false;
    }
    true
}



pub fn apply_heating(client: &Mosquitto, control_nodes: &ControlNodes, states: &States, config: &Config) -> u16
{
    let now = Local::now();
    let mut count = 0;
    for (control_name, control_node) in control_nodes {

        let mut dt_first_zone_started = None;
        let mut dt_last_zone_finished = None;

        let control_state: Option<&PinCollection> = states.get(control_name).and_then(|col| col.get(&control_node.control_pin));

        for (zone_name, zone) in &control_node.zones {

            let expected_temperature: Option<Temperature> = zone.get_expected_temperature(&now.time());

            let temperatures: Option<&PinCollection> = states.get(zone_name).and_then(|zst| zst.get(&zone.sensor_pin));
            let current_state: Option<&PinCollection> = states.get(control_name).and_then(|control| control.get(&zone.control_pin));

            let avg_temp: Option<Temperature> = temperatures.map(|col| col.get_average_temperature());
            let state_on = current_state.map(PinCollection::is_on).unwrap_or(false);
            let state_off = current_state.map(PinCollection::is_off).unwrap_or(false);

            let turn_on = (state_off || current_state.is_none())
                && avg_temp.is_some()
                && expected_temperature.is_some()
                && avg_temp.clone().unwrap() < (expected_temperature.clone().unwrap() - Temperature { value: config.temperature_drop_wait });

            let turn_off = state_on
                && avg_temp.is_some()
                && (expected_temperature.is_some() && avg_temp.clone().unwrap() > expected_temperature.clone().unwrap()
                    || expected_temperature.is_none()
                );
                

            if state_on {
                dt_first_zone_started = more_recent_date(current_state.and_then(|s| s.get_last_changed_dt()), dt_first_zone_started);
            } else if state_off {
                dt_last_zone_finished = more_recent_date(dt_last_zone_finished, current_state.and_then(|s| s.get_last_changed_dt()));
            }

            if turn_on {
                let diff = (expected_temperature.unwrap() - avg_temp.unwrap()).abs();
                let value = if (diff <= Temperature { value: config.min_temperature_diff_for_pwm }) {
                    percent_to_analog(config.min_pwm_state)
                } else if (diff < Temperature { value: 1f32} ) {
                    percent_to_analog((diff.value * 100f32) as u8)
                } else {
                    percent_to_analog(100)
                };
                
                count += send_to_zone(client, zone.control_pin, value, &config.name, &control_name) as u16;
     
            } else if turn_off {

                if control_state.is_some() {
                    count += send_to_zone(client, zone.control_pin, 0, &config.name, control_name) as u16;
                }
            }
        }

        if control_node.control_pin > 0 {
            if let Some(dt) = dt_first_zone_started {
                let can_start = Local::now() - dt > chrono::Duration::seconds(config.acctuator_warmup_time as i64);
                if can_start && control_state.is_some() && control_state.unwrap().is_off() {
                    count += send_to_zone(client, control_node.control_pin, 1, &config.name, control_name) as u16;
                }
            } else if let Some(dt) = dt_last_zone_finished {
                let can_stop = Local::now() - dt > chrono::Duration::seconds(config.heater_pump_stop_time as i64);
                if can_stop && control_state.is_some() && control_state.unwrap().is_on() {
                    count += send_to_zone(client, control_node.control_pin, 0, &config.name, control_name) as u16;
                }
            }
        }

    }
    count
}


pub fn print_info(control_nodes, &ControlNodes, repository: &StatePinRepository)
{
    for (control_name, node) in control_nodes {
        for (zone_name, zone) in node.zones {
            debug!("Node: {} Pin: {} Collection: {:?}", node, pin, col.get_last_changed_value(), col.get_last_changed_dt());
            debug!("Node: {} Pin: {} Collection: {:?}", node, pin, col.get_last_changed_value(), col.get_last_changed_dt());
        }
    }
}



