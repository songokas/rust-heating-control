#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

#[path = "../config.rs"]
pub mod config;
#[path = "../helper.rs"]
#[macro_use]
pub mod helper;
#[path = "../zone.rs"]
pub mod zone;
#[path = "../repository.rs"]
pub mod repository;
#[path = "../schema.rs"]
pub mod schema;


use std::fs::File;
use std::io::{Read, BufReader, BufWriter};
use crate::config::{ControlNodes, FullConfig, Settings};
use rocket::State;
use rocket_contrib::json::{Json, JsonValue};
use clap::{App, load_yaml};
use rocket::response::content::Html;
use diesel::{SqliteConnection, Connection};
use std::collections::HashMap;
use arduino_mqtt_pin::pin::PinState;
use serde::{Serialize, Deserialize};
use chrono::{Local, Duration};
use crate::repository::{PinStateRepository};
use derive_new::new;

#[derive(new)]
struct UiSettings
{
    pub config_path: String,
    pub html_path: String,
    pub db_path: String
}

#[derive(Serialize, Deserialize, Debug, new, PartialEq)]
struct TimeInfo
{
    start: i64,
    end: i64
}

// assumes
// PinState { value: 0 } PinState { value: 1 } - PinState { value: 0 }
// or
// PinState { value: 1 } - PinState { value: 0 } PinState { value: 1 }
fn time_info_from_arr(arr: &[PinState]) -> Vec<TimeInfo>
{
    let mut times = Vec::new();
    let mut it = arr.iter();
    loop {
        if let Some(s1) = it.next() {
            if s1.is_on() {
                times.push(TimeInfo { start: s1.dt.timestamp(), end: 0 })
            } else {
                if let Some(s2) = it.next() {
                    if s2.is_on() {
                        times.push(TimeInfo { start: s2.dt.timestamp(), end: s1.dt.timestamp() })
                    } else {
                        times.push(TimeInfo { end: s1.dt.timestamp(), start: 0 });
                        times.push(TimeInfo { end: s2.dt.timestamp(), start: 0 });
                    }
                } else {
                    times.push(TimeInfo { end: s1.dt.timestamp(), start: 0 })
                }
            }
        } else {
            break;
        }
    }
    times
}

#[derive(Serialize, Deserialize)]
struct HeaterInfo
{
    on: bool,
    times: Vec<TimeInfo>
}

#[derive(new, Serialize, Deserialize)]
struct ZoneInfo
{
    name: String,
    control_pin: u8,
    state: bool,
    current_temperature: Option<f32>,
    expected_temperature: Option<f32>,
    states: Vec<TimeInfo>,
    dtc: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct ControlInfo
{
    name: String,
    zones: Vec<ZoneInfo>
}

#[derive(Serialize, Deserialize)]
struct Info
{
    heater: HeaterInfo,
    controls: Vec<ControlInfo>
}

fn load_info(db_path: &str, config: &Settings, control_nodes: &ControlNodes) -> Result<Info, String>
{
    let connection = SqliteConnection::establish(db_path)
        .map_err(|e| format!("Unable to connect to db: {:?}", e))?;
    let repository = PinStateRepository::new(&connection);
    let last_heater_state = repository.get_last_pin_state(&config.heater_control_name(), config.heater_control_pin())
        .map(|s| s.is_on()).unwrap_or(false);
    let last_heater_times = repository.get_pin_changes(&config.heater_control_name(), config.heater_control_pin(), 6)
        .map(|arr| time_info_from_arr(&arr)).unwrap_or(vec![]);
    let now = Local::now();
    let mut control_arr = Vec::new();
    for (control_name, control_node) in control_nodes {
        let mut zones = Vec::new();

        for (zone_name, zone) in &control_node.zones {
            let states = repository.get_pin_changes(control_name, zone.control_pin, 6)
                .map(|arr| time_info_from_arr(&arr)).unwrap_or(vec![]);
            let (on, timestamp) = repository.get_last_pin_state(control_name, zone.control_pin)
                .map(|s| (s.is_on(), Some(s.dt.timestamp()))).unwrap_or((false, None));
            let zone_info = ZoneInfo::new(
                zone_name.to_owned(),
                zone.control_pin,
                on,
                repository.get_average_temperature(zone_name, zone.sensor_pin, &(now - Duration::hours(1))).map(|t| t.value),
                zone.get_expected_temperature(&now.time()).map(|t| t.value),
                states,
                timestamp
            );
            zones.push(zone_info);
        }
        control_arr.push(ControlInfo { name: control_name.to_owned(), zones });
    }
    Ok(Info { heater: HeaterInfo { on: last_heater_state, times: last_heater_times}, controls: control_arr })

}

#[get("/")]
fn show_config(settings: State<UiSettings>) -> Result<Html<String>, String>
{
    let yaml_file = File::open(&settings.config_path).map_err(|_| "Unable to open config file")?;
    let reader = BufReader::new(yaml_file);
    let full_config: FullConfig = serde_yaml::from_reader(reader).map_err(|_| "Unable to parse error")?;

    let config_json = serde_json::to_string(&full_config).map_err(|_| "Failed to serialize config to string")?;
    let data = load_info(&settings.db_path,&Settings::new(full_config.general.clone()), &full_config.controls)?;
    let info_json = serde_json::to_string(&data).map_err(|_| "Failed to serialize info to string")?;

    let mut html_file = File::open(&settings.html_path).map_err(|_| "Unable to open html file")?;
    let mut contents = String::new();
    html_file.read_to_string(&mut contents).map_err(|_| "Unable to read html file")?;
    Ok(Html(contents.replace("{insert_settings}", &config_json).replace("{insert_info}", &info_json)))
}

#[post("/", format = "json", data = "<config>")]
fn update_config(config: Json<FullConfig>, settings: State<UiSettings>) -> Result<JsonValue, JsonValue>
{
    let json = serde_json::to_string(&config.into_inner()).map_err(|_| json!({"error": "Failed to serialize to string"}))?;
    let full_config: FullConfig = serde_yaml::from_str(&json).map_err(|_| json!({"error": "Unable to parse error"}))?;
    let yaml_file = File::create(&settings.config_path).map_err(|_| json!({"error": "Unable to open file"}))?;
    let writer = BufWriter::new(yaml_file);
    serde_yaml::to_writer(writer, &full_config).map_err(|_| json!({"error": "Unable to write to file"}))?;
    Ok(json!({
        "success": true,
    }))
}

embed_migrations!("migrations");

fn main() {
    let yaml = load_yaml!("ui.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let config_path = matches.value_of("config").unwrap_or("config.yml");
    let html_path = matches.value_of("html").unwrap_or("index.html");
    let db_path = matches.value_of("db").unwrap_or("pins.sqlite3");
    rocket::ignite()
        .manage(UiSettings::new(config_path.to_owned(), html_path.to_owned(), db_path.to_owned()))
        .mount("/", routes![show_config, update_config]).launch();
}


#[cfg(test)]
pub mod test
{
    use speculate::speculate;
    use super::*;
    use chrono::{TimeZone, NaiveTime};
    use arduino_mqtt_pin::pin::PinValue;

    speculate! {
        describe "ui tests"
        {
            it "should provide time info when on"
            {
                let states = [
                    PinState::new(1, PinValue::Analog(255), Local.ymd(2019, 8, 2).and_hms(8, 0, 0), None),
                    PinState::new(1, PinValue::Analog(0), Local.ymd(2019, 8, 1).and_hms(8, 0, 0), None),
                    PinState::new(1, PinValue::Analog(1), Local.ymd(2019, 8, 1).and_hms(7, 0, 0), None),
                    PinState::new(1, PinValue::Analog(0), Local.ymd(2019, 6, 1).and_hms(8, 0, 0), None),
                    PinState::new(1, PinValue::Analog(1), Local.ymd(2019, 5, 1).and_hms(7, 0, 0), None),
                ];

                let expected = [
                    TimeInfo::new(1564722000, 0),
                    TimeInfo::new(1564632000, 1564635600),
                    TimeInfo::new(1556683200, 1559365200)
                ];
                let result = time_info_from_arr(&states);

                assert_eq!(expected, result[..]);
            }

            it "should provide time info when off"
            {
                let states = [
                    PinState::new(1, PinValue::Analog(0), Local.ymd(2019, 8, 2).and_hms(8, 0, 0), None),
                    PinState::new(1, PinValue::Analog(1), Local.ymd(2019, 8, 1).and_hms(8, 0, 0), None),
                    PinState::new(1, PinValue::Analog(0), Local.ymd(2019, 8, 1).and_hms(7, 0, 0), None),
                    PinState::new(1, PinValue::Analog(1), Local.ymd(2019, 6, 1).and_hms(8, 0, 0), None),
                    PinState::new(1, PinValue::Analog(0), Local.ymd(2019, 5, 1).and_hms(7, 0, 0), None),
                ];

                let expected = [
                    TimeInfo::new(1564635600, 1564722000),
                    TimeInfo::new(1559365200, 1564632000),
                    TimeInfo::new(0, 1556683200)
                ];
                let result = time_info_from_arr(&states);

                assert_eq!(expected, result[..]);
            }

            it "should handle missing states"
            {
                let states = [
                    PinState::new(1, PinValue::Analog(0), Local.ymd(2019, 8, 2).and_hms(8, 0, 0), None),
                    PinState::new(1, PinValue::Analog(0), Local.ymd(2019, 8, 1).and_hms(8, 0, 0), None),
                    PinState::new(1, PinValue::Analog(1), Local.ymd(2019, 8, 1).and_hms(7, 0, 0), None),
                    PinState::new(1, PinValue::Analog(1), Local.ymd(2019, 6, 1).and_hms(8, 0, 0), None),
                    PinState::new(1, PinValue::Analog(1), Local.ymd(2019, 5, 1).and_hms(7, 0, 0), None),
                ];

                let expected = [
                    TimeInfo::new(0, 1564722000),
                    TimeInfo::new(0, 1564635600),
                    TimeInfo::new(1564632000, 0),
                    TimeInfo::new(1559365200, 0),
                    TimeInfo::new(1556683200, 0)
                ];
                let result = time_info_from_arr(&states);

                assert_eq!(expected, result[..]);
            }
        }
    }
}
