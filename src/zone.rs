use chrono::{NaiveTime};
use std::vec::Vec;
use yaml_rust::{Yaml};
use serde::{Serialize, Deserialize};
use derive_new::{new};
use arduino_mqtt_pin::pin::Temperature;

#[derive(Debug, new, Serialize, Deserialize)]
pub struct Interval
{
    #[serde(with = "serde_naive_time")]
    start: NaiveTime,
    #[serde(with = "serde_naive_time")]
    end: NaiveTime,
    #[serde(with = "serde_temperature")]
    expected_temperature: Temperature
}

mod serde_temperature {
    use super::*;
    use serde::{Serializer, Deserializer};

    pub fn serialize<S: Serializer>(x: &Temperature, s: S) -> Result<S::Ok, S::Error>
    {
        s.serialize_f32(x.value)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Temperature, D::Error>
    {
        let temp: f32 = Deserialize::deserialize(deserializer)?;
        Ok(Temperature::new(temp))
//        let temp: String = Deserialize::deserialize(deserializer)?;
//        Ok(Temperature::from_str(&temp).map_err(|e| D::Error::custom(e))?)
    }
}

mod serde_naive_time {
    use super::*;
    use serde::{Serializer, Deserializer, de::Error};

    pub fn serialize<S: Serializer>(time: &NaiveTime, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{}", time.format("%H:%M")))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<NaiveTime, D::Error> {
        let time: String = Deserialize::deserialize(deserializer)?;
        Ok(NaiveTime::parse_from_str(&time, "%H:%M").map_err(|_| D::Error::custom("failed to parse naive time"))?)
    }
}

#[derive(Debug, new, Serialize, Deserialize)]
pub struct Zone
{
    #[serde(default)]
    pub name: String,
    #[serde(default)]
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
          if *now >= time.start && *now < time.end {
              return Some(time.expected_temperature.clone());
          }
        }
        None
        
    }
}

#[cfg(test)]
mod tests {

    use speculate::speculate;
    use super::*;

    fn times_expected_data() -> Vec<(&'static str, NaiveTime, f32)>
    {
        vec![
            ("temperature 20 1", NaiveTime::from_hms(8, 0, 0), 20.0),
            ("temperature 20 2", NaiveTime::from_hms(8, 30, 0), 20.0),
            ("temperature 20 3", NaiveTime::from_hms(8, 59, 59), 20.0),
            ("temperature 20 4", NaiveTime::from_hms(23, 2, 59), 30.5),
        ]
    }

    speculate! {
        describe "zone"
        {
            before {
                let intervals = vec![
                    Interval::new(NaiveTime::from_hms(8, 0, 0), NaiveTime::from_hms(9, 0, 0), Temperature::new(20.0)),
                    Interval::new(NaiveTime::from_hms(23, 1, 0), NaiveTime::from_hms(23, 3, 3), Temperature::new(30.5))
                ];
                let zone = Zone::new(String::from("zone1"), 1, intervals, 2);
            }

            it "should have temperature"
            {
                for (e, time, temp) in times_expected_data() {
                    assert_eq!(zone.get_expected_temperature(&time).expect(e), Temperature::new(temp));
                }
            }

            it "should not have temperature"
            {
                for hour in vec![9, 20] {
                    assert!(zone.get_expected_temperature(&NaiveTime::from_hms(hour, 0, 0)).is_none());
                }
            }
        }
    }
}