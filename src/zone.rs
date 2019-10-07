use chrono::{NaiveTime};
use std::vec::Vec;
use yaml_rust::{Yaml};

use arduino_mqtt_pin::pin::Temperature;

#[derive(Debug)]
struct Interval
{
    start: NaiveTime,
    end: NaiveTime,
    expected_temperature: Temperature
}

#[derive(Debug)]
pub struct Zone
{
    pub name: String,
    pub sensor_pin: u8,
    times: Vec<Interval>,
    pub control_pin: u8

}

impl Zone
{

    pub fn from_yaml(name: &str, yaml: &Yaml) -> Result<Zone, String>
    {
        let sensor_pin = yaml["sensor_pin"].as_i64().ok_or("Zone yaml invalid sensor_pin")? as u8;
        let control_pin = yaml["control_pin"].as_i64().ok_or(format!("Zone yaml invalid control_pin {:?}", yaml["control_pin"]))? as u8;
        let mut v = Vec::new();
        for time in yaml["times"].as_vec().ok_or(format!("Zone yaml invalid times {}", name))? {
            let start = NaiveTime::parse_from_str(
                &format!("{}:00", time["start"].as_str().ok_or(format!("Zone yaml invalid times.start in {}", name))?),
                "%H:%M:%S"
            ).map_err(|_| format!("Zone yaml invalid time format times.start {}", name))?;
            let end = NaiveTime::parse_from_str(
                &format!("{}:00", time["end"].as_str().ok_or(format!("Zone yaml invalid times.end in {}", name))?),
                "%H:%M:%S"
            ).map_err(|_| format!("Zone yaml invalid time format times.end in {}", name))?;
            v.push(Interval {
                start,
                end,
                expected_temperature: Temperature::from_yaml(&time["expected_temperature"]).ok_or(format!("Zone yaml invalid expected_temperature in {}", name))?
            });
        }
        Ok(Zone {name: name.to_string(), sensor_pin, control_pin, times: v})
    }

    pub fn get_expected_temperature(&self, now: &NaiveTime) -> Option<Temperature>
    {
        for time in &self.times {
          if now > &time.start && now < &time.end {
              return Some(time.expected_temperature.clone());
          }
        }
        None
        
    }
}
