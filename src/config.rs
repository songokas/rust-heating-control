use std::collections::HashMap;
use yaml_rust::{Yaml};

use arduino_mqtt_pin::pin::PinCollection;
use crate::zone::Zone;

pub type ControlNodes = HashMap<String, ControlNode>;
pub type States = HashMap<String, HashMap<u8, PinCollection>>;

#[derive(Debug)]
pub struct ControlNode
{
    pub name: String,
    pub control_pin: u8,
    pub zones: HashMap<String, Zone>
}

#[derive(Debug)]
pub struct Config
{
    pub name: String,
    pub host: String,
    pub acctuator_warmup_time: u16,
    pub heater_pump_stop_time: u16,
    pub constant_temperature_expected: f32,
    pub min_pwm_state: u8,
    pub min_temperature_diff_for_pwm: f32,
    pub temperature_drop_wait: f32
}

impl Config
{
    pub fn from_yaml(yaml: &Yaml) -> Result<Config, &str>
    {
        let name = yaml["name"].as_str().ok_or("yaml missing name")?;
        let host = yaml["host"].as_str().ok_or("yaml missing host")?;
        let acctuator_warmup_time = yaml["acctuator_warmup_time"].as_i64().ok_or("yaml missing acctuator_warmup_time")? as u16;
        let heater_pump_stop_time = yaml["heater_pump_stop_time"].as_i64().ok_or("yaml missing heater_pump_stop_time")? as u16;
        let constant_temperature_expected = yaml["constant_temperature_expected"].as_f64().ok_or("yaml missing constant_temperature_expected")? as f32;
        let min_pwm_state = yaml["min_pwm_state"].as_i64().ok_or("ymal missing min_pwm_state")? as u8;
        let min_temperature_diff_for_pwm = yaml["min_temperature_diff_for_pwm"].as_f64().ok_or("yaml missing min_temperature_diff_for_pwm")? as f32;
        let temperature_drop_wait = yaml["temperature_drop_wait"].as_f64().ok_or("yaml missing temperature_drop_wait")? as f32;
        Ok(Config {
            name: name.to_string(),
            host: host.to_string(),
            acctuator_warmup_time,
            heater_pump_stop_time,
            constant_temperature_expected,
            min_pwm_state,
            min_temperature_diff_for_pwm,
            temperature_drop_wait
        })

    }
}
