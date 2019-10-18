use crate::config::{Zones, ControlNodes, Config};
use std::collections::HashMap;
use arduino_mqtt_pin::pin::PinValue;
use crate::repository::PinStateRepository;
use crate::deciders::{HeaterDecider, ZoneStateDecider};
use chrono::{DateTime, Local};

pub type PinChanges = HashMap<String, HashMap<u8, PinValue>>;

#[derive(new)]
pub struct StateRetriever<'a>
{
    repository: &'a PinStateRepository,
    heater_decider: &'a HeaterDecider<'a>,
    zone_decider: &'a ZoneStateDecider<'a>,
    config: &'a Config
}

impl StateRetriever<'_>
{
    pub fn get_zone_pins_to_change(&self, control_name: &str, zones: &Zones, now: &DateTime<Local>) -> HashMap<u8, PinValue>
    {
        let mut zone_changes: HashMap<u8, PinValue> = HashMap::new();
        for (zone_name, zone) in zones {
            if let Some(last_state) = self.repository.get_last_changed_pin_state(control_name, zone.control_pin) {
                if let Some(avg_temp) = self.repository.get_average_temperature(zone_name, zone.sensor_pin) {
                    self.zone_decider.get_value_to_change_to(last_state, zone, &avg_temp, now)
                        .map(|value| zone_changes.insert(zone.control_pin, value));
                } else if last_state.is_on() {
                    zone_changes.insert(zone.control_pin, PinValue::Analog(0u16));
                }
            }
        }
        zone_changes
    }

    pub fn get_pins_expected_to_change(&self, control_nodes: &ControlNodes, now: &DateTime<Local>) -> PinChanges
    {
        let mut control_changes: PinChanges = PinChanges::new();

        let current_state = self.repository.get_last_changed_pin_state(&self.config.heater_control_name, self.config.heater_control_pin);
        if let Some(state) = current_state {
            if state.is_on() && self.all_zones_should_be_off(control_nodes) {
                self.turn_heater(&mut control_changes, false);
                return control_changes;
            } else if !self.heater_decider.can_turn_zones_off(state, now) {
                return control_changes;
            }
        }

        for (control_name, control_node) in control_nodes {
            let zone_changes = self.get_zone_pins_to_change(control_name, &control_node.zones, now);
            if zone_changes.len() > 0 {
                control_changes.insert(control_name.clone(), zone_changes);
            }
        }

        if let Some(state) = current_state {
            if !state.is_on() && self.heater_decider.should_be_on(control_nodes, now) {
                self.turn_heater(&mut control_changes, true);
            }
        }

        control_changes
    }

    fn all_zones_should_be_off(&self, control_nodes: &ControlNodes) -> bool
    {
        let now = Local::now();
        for (control_name, control_node) in control_nodes {
            for (zone_name, zone) in &control_node.zones {
                if let Some(last_state) = self.repository.get_last_changed_pin_state(control_name, zone.control_pin) {
                    if let Some(avg_temp) = self.repository.get_average_temperature(zone_name, zone.sensor_pin) {
                        if self.zone_decider.should_be_on(last_state, zone, &avg_temp, now) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    fn turn_heater(&self, control_changes: &mut PinChanges, value: bool)
    {
        control_changes.entry(self.config.heater_control_name.clone()).and_modify(|zone_changes| {
            zone_changes.insert(self.config.heater_control_pin, PinValue::Digital(value));
        }).or_insert_with(|| {
            let mut zone_changes = HashMap::new();
            zone_changes.insert(self.config.heater_control_pin, PinValue::Digital(value));
            zone_changes
        });
    }
}
