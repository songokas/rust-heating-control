use std::collections::HashMap;
use log::{error, debug};
use std::time::{UNIX_EPOCH};

use crate::zone::Zone;
use std::fs::{metadata, File};
use std::io::{Error, ErrorKind, BufReader};
use std::cell::{RefCell};
use serde::{Serialize, Deserialize};
use derive_new::{new};

pub type ControlNodes = HashMap<String, ControlNode>;
pub type Zones = HashMap<String, Zone>;

#[derive(Serialize, Deserialize)]
pub struct FullConfig
{
    pub general: Config,
    pub controls: ControlNodes
}

#[derive(Debug, new, Serialize, Deserialize)]
pub struct ControlNode
{
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub control_pin: u8,
    pub zones: Zones
}

#[derive(Debug)]
pub struct Settings
{
    config: RefCell<Config>
}

impl Settings
{
    pub fn new(config: Config) -> Settings
    {
        Settings { config: RefCell::new(config) }
    }

    pub fn replace(&self, config: Config)
    {
        self.config.replace(config);
    }

    pub fn name(&self) -> String
    {
       self.config.borrow().name.clone()
    }

    pub fn host(&self) -> String
    {
        self.config.borrow().host.clone()
    }

    pub fn heater_control_name(&self) -> String
    {
        self.config.borrow().heater_control_name.clone()
    }

    pub fn heater_control_pin(&self) -> u8
    {
        self.config.borrow().heater_control_pin
    }

    pub fn acctuator_warmup_time(&self) -> u16
    {
        self.config.borrow().acctuator_warmup_time
    }

    pub fn heater_pump_stop_time(&self) -> u16
    {
        self.config.borrow().heater_pump_stop_time
    }

    pub fn constant_temperature_expected(&self) -> f32
    {
        self.config.borrow().constant_temperature_expected
    }

    pub fn min_pwm_state(&self) -> u8
    {
        self.config.borrow().min_pwm_state
    }

    pub fn min_temperature_diff_for_pwm(&self) -> f32
    {
        self.config.borrow().min_temperature_diff_for_pwm
    }

    pub fn temperature_drop_wait(&self) -> f32
    {
        self.config.borrow().temperature_drop_wait
    }

    pub fn version(&self) -> u64
    {
        self.config.borrow().version
    }
}


#[derive(Debug, new, Serialize, Deserialize, Clone)]
pub struct Config
{
    name: String,
    host: String,
    heater_control_name: String,
    heater_control_pin: u8,
    #[new(value = "300")]
    acctuator_warmup_time: u16,
    #[new(value = "600")]
    heater_pump_stop_time: u16,
    #[new(value = "20.0")]
    constant_temperature_expected: f32,
    #[new(value = "30")]
    min_pwm_state: u8,
    #[new(value = "0.3")]
    min_temperature_diff_for_pwm: f32,
    #[new(value = "0.7")]
    temperature_drop_wait: f32,
    #[new(value = "0")]
    #[serde(default)]
    version: u64
}

pub fn load_config(config_path: &str, verbosity: u8) -> Result<(Config, ControlNodes), Error>
{

    let yaml_file = File::open(&config_path)
        .map_err(|err| error!("{:?}", err))
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Unable to open yaml file"))?;
    let reader = BufReader::new(yaml_file);
    let mut full_config: FullConfig = serde_yaml::from_reader(reader)
        .map_err(|err| error!("{:?}", err))
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Unable to parse yaml file"))?;

    debug!("Config loaded: {} Verbosity: {}", config_path, verbosity);

    let version =  metadata(config_path)
        .and_then(|meta| meta.modified())
        .map(|stime| if let Ok(dur) = stime.duration_since(UNIX_EPOCH) { dur.as_secs() } else { 0 })
        .unwrap_or(0);
    if full_config.general.version != version {
        full_config.general.version = version;
    }
    Ok((full_config.general, full_config.controls))
}

pub fn has_config_changed(config_path: &str, version: u64) -> bool
{
    if let Ok(meta) = metadata(config_path) {
        if let Ok(stime) = meta.modified() {
            if let Ok(dur) = stime.duration_since(UNIX_EPOCH) {
                return version < dur.as_secs();
            }
        }
    }
    false
}

#[cfg(test)]
mod tests
{

    use speculate::speculate;
    use super::*;
    use serde_yaml;
    use serde_json;

    speculate! {
        describe "config serialization"
        {
            it "should serialize full config"
            {
                let contents = "
general:
  host: 192.168.0.140

  name: sildymas

  # how long it takes for acctuator to warm up in secs
  acctuator_warmup_time: 180

  # how long it takes for pump to stop working in secs
  heater_pump_stop_time: 600

  # ignore zone config and expect this temperature when enabled
  constant_temperature_expected: 18.0

  # min value for pwm pin in percent
  min_pwm_state: 30

  # if the temperature difference is less then min_temperature_diff_for_pwm use min_pwm_state
  min_temperature_diff_for_pwm: 0.5

  # when temperature reaches its expected value wait for it to drop temperature_drop_wait to turn acctuator back on
  temperature_drop_wait: 0.7
  heater_control_name: main_control
  heater_control_pin: 83

controls:
  main_control:
    path: sildymas/nodes/main
    control_pin: 83
    zones:
      salionas:
        times:
          - start: 4:00
            end: 21:00
            expected_temperature: 21.0
          - start: 4:00
            end: 21:00
            expected_temperature: 21.0
        sensor_pin: 2
        control_pin: 4

  slave_control:
    path: sildymas/nodes/slave
    zones:
      miegamasis:
        times:
          - start: 2:00
            end: 23:00
            expected_temperature: 20.5
        control_pin: 10
        sensor_pin: 2
      vaiku:
        times:
          - start: 2:00
            end: 23:00
            expected_temperature: 20.5
        control_pin: 9
        sensor_pin: 2
                ";
                let config: FullConfig = serde_yaml::from_str(&contents).unwrap();
                let json = serde_json::to_string(&config).unwrap();
            }
        }
    }
}