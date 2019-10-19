use std::collections::HashMap;
use yaml_rust::{Yaml, YamlLoader};
use log::{error};

use crate::zone::Zone;
use std::fs::File;
use std::io::{Error, ErrorKind, Read};

pub type ControlNodes = HashMap<String, ControlNode>;
pub type Zones = HashMap<String, Zone>;

#[derive(Debug, new)]
pub struct ControlNode
{
    pub name: String,
    pub control_pin: u8,
    pub zones: Zones
}

#[derive(Debug, new)]
pub struct Config
{
    pub name: String,
    pub host: String,
    pub heater_control_name: String,
    pub heater_control_pin: u8,
    #[new(value = "300")]
    pub acctuator_warmup_time: u16,
    #[new(value = "600")]
    pub heater_pump_stop_time: u16,
    #[new(value = "20.0")]
    pub constant_temperature_expected: f32,
    #[new(value = "30")]
    pub min_pwm_state: u8,
    #[new(value = "0.3")]
    pub min_temperature_diff_for_pwm: f32,
    #[new(value = "0.7")]
    pub temperature_drop_wait: f32,
}

impl Config
{
    pub fn from_yaml(yaml: &Yaml) -> Result<Config, String>
    {
        let name = yaml["name"].as_str().ok_or("yaml missing name")?;
        let host = yaml["host"].as_str().ok_or("yaml missing host")?;
        let acctuator_warmup_time = yaml["acctuator_warmup_time"].as_i64().ok_or("yaml missing acctuator_warmup_time")? as u16;
        let heater_pump_stop_time = yaml["heater_pump_stop_time"].as_i64().ok_or("yaml missing heater_pump_stop_time")? as u16;
        let constant_temperature_expected = yaml["constant_temperature_expected"].as_f64().ok_or("yaml missing constant_temperature_expected")? as f32;
        let min_pwm_state = yaml["min_pwm_state"].as_i64().ok_or("ymal missing min_pwm_state")? as u8;
        let min_temperature_diff_for_pwm = yaml["min_temperature_diff_for_pwm"].as_f64().ok_or("yaml missing min_temperature_diff_for_pwm")? as f32;
        let temperature_drop_wait = yaml["temperature_drop_wait"].as_f64().ok_or("yaml missing temperature_drop_wait")? as f32;
        let (heater_control_name, heater_control_pin) =  Config::get_control_names(yaml)?;

        Ok(Config {
            name: name.to_string(),
            host: host.to_string(),
            acctuator_warmup_time,
            heater_pump_stop_time,
            constant_temperature_expected,
            min_pwm_state,
            min_temperature_diff_for_pwm,
            temperature_drop_wait,
            heater_control_name,
            heater_control_pin
        })
    }

    fn get_control_names(yaml: &Yaml) -> Result<(String, u8), String>
    {
        let controls = yaml["controls"].as_hash();
        if !controls.is_some() {
            return Err("Failed to parse controls".to_string())
        }
        for (key, node) in controls.unwrap() {
            if !key.as_str().is_some() {
                continue;
            }
            let name = key.as_str().unwrap();
            let control_pin = node["control_pin"].as_i64().unwrap_or(0) as u8;
            if control_pin > 0 {
                return Ok((name.to_string(), control_pin));
            }
        }
        Err("Main control not found".to_string())
    }


}

pub fn create_nodes(yaml: &Yaml) -> Result<ControlNodes, String>
{
    let mut control_nodes = ControlNodes::new();
    let controls = yaml["controls"].as_hash();
    if !controls.is_some() {
       return Err("Failed to parse controls".to_string())
    }
    for (key, node) in controls.unwrap() {
        if !key.as_str().is_some() {
            continue;
        }

        let yaml_zones = node["zones"].as_hash();
        if !yaml_zones.is_some() {
            return Err("Failed to parse zones".to_string())
        }

        let mut zones: HashMap<String, Zone> = HashMap::new();
        for (zone_name, zone) in yaml_zones.unwrap() {
            if !zone_name.as_str().is_some() {
                continue;
            }
            let z = Zone::from_yaml(zone_name.as_str().unwrap(), zone)?;
            zones.insert(z.name.clone(), z);
        }
        let name = key.as_str().unwrap();
        let control_pin = node["control_pin"].as_i64().unwrap_or(0) as u8;

        control_nodes.insert(name.to_string(), ControlNode {name: name.to_string(), control_pin, zones});
    }
    return Ok(control_nodes);
}

pub fn load_config(config_path: &str, verbosity: u8) -> Result<(Config, ControlNodes), Error>
{
    let mut yaml_file = File::open(config_path)?;
    let mut contents = String::new();
    yaml_file.read_to_string(&mut contents)?;

    println!("Config loaded: {} Verbosity: {}", config_path, verbosity);

    let yaml_config = YamlLoader::load_from_str(&contents)
        .map_err(|err| error!("{:?}", err))
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Unable to parse yaml file"))?;

    let config = Config::from_yaml(&yaml_config[0])
        .map_err(|s| { error!("{}", s); s })
        .map_err(|err| Error::new(ErrorKind::InvalidData, "Unable to parse config section"))?;

    let control_nodes = create_nodes(&yaml_config[0])
        .map_err(|s| { println!("{}", s); s })
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Unable to create control configuration"))?;
    Ok((config, control_nodes))
}
