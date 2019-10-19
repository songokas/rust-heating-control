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
                    self.zone_decider.get_value_to_change_to(&last_state, zone, &avg_temp, now)
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
        let current_state = self.repository.get_last_changed_pin_state(&self.config.heater_control_name, self.config.heater_control_pin);
        if let Some(state) = current_state.clone() {
            if state.is_on() && self.all_zones_should_be_off(control_nodes, now) {
                return self.turn_heater(false);
            } else if !self.heater_decider.can_turn_zones_off(&state, now) {
                return PinChanges::new();;
            }
        }

        let mut control_changes: PinChanges = PinChanges::new();
        for (control_name, control_node) in control_nodes {
            let zone_changes = self.get_zone_pins_to_change(control_name, &control_node.zones, now);
            if zone_changes.len() > 0 {
                control_changes.insert(control_name.clone(), zone_changes);
            }
        }
        if control_changes.len() > 0 {
            return control_changes;
        }

        if let Some(state) = current_state {
            if !state.is_on() && self.heater_decider.should_be_on(control_nodes, now) {
                return self.turn_heater(true);
            }
        }

        PinChanges::new()
    }

    fn all_zones_should_be_off(&self, control_nodes: &ControlNodes, now: &DateTime<Local>) -> bool
    {
        for (control_name, control_node) in control_nodes {
            for (zone_name, zone) in &control_node.zones {
                if let Some(last_state) = self.repository.get_last_changed_pin_state(control_name, zone.control_pin) {
                    if let Some(avg_temp) = self.repository.get_average_temperature(zone_name, zone.sensor_pin) {
                        if self.zone_decider.should_be_on(&last_state, zone, &avg_temp, &now) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    fn turn_heater(&self, value: bool) -> PinChanges
    {
        let mut control_changes: PinChanges = PinChanges::new();
        control_changes.entry(self.config.heater_control_name.clone()).and_modify(|zone_changes| {
            zone_changes.insert(self.config.heater_control_pin, PinValue::Digital(value));
        }).or_insert_with(|| {
            let mut zone_changes = HashMap::new();
            zone_changes.insert(self.config.heater_control_pin, PinValue::Digital(value));
            zone_changes
        });
        control_changes
    }
}

#[cfg(test)]
mod test_deciders
{
    use super::*;
    use chrono::{TimeZone, NaiveTime};
    use crate::repository::test_repository::{create_nodes, create_repository};
    use crate::deciders::{TemperatureStateDecider, HeaterDecider, ZoneStateDecider};
    use arduino_mqtt_pin::pin::{PinState, PinOperation};

    speculate! {
        describe "state changes"
        {
            before
            {
                let config = Config::new("test".to_owned(), "host".to_owned(), "main".to_owned(), 34);
                let temp_decider = TemperatureStateDecider::new(&config);
                let repository = create_repository();
                let heater_decider = HeaterDecider::new(&repository, &config);
                let zone_decider = ZoneStateDecider::new(&temp_decider, &config);
                let state_retriever = StateRetriever::new(&repository, &heater_decider, &zone_decider, &config);
            }

            it "should change pins"
            {
                let nodes = create_nodes();

                assert!(!state_retriever.all_zones_should_be_off(&nodes, &Local.ymd(2019, 8, 2).and_hms(8, 20, 0)));

                let pins = state_retriever.get_pins_expected_to_change(&nodes, &Local.ymd(2019, 8, 2).and_hms(8, 20, 0));
                assert_eq!(pins.len(), 0, "{:?}", pins);

                assert!(state_retriever.all_zones_should_be_off(&nodes, &Local.ymd(2019, 8, 2).and_hms(9, 3, 0)));
                let expected: PinChanges = map!{ "main".to_owned() => map!{ 34 =>  PinValue::Digital(false) }};
                let pins = state_retriever.get_pins_expected_to_change(&nodes, &Local.ymd(2019, 8, 2).and_hms(9, 3, 0));
                assert_eq!(pins, expected);

                let expected: PinChanges = map!{ "main".to_owned() => map!{ 34 =>  PinValue::Digital(false) }};
                let pins = state_retriever.get_pins_expected_to_change(&nodes, &Local.ymd(2019, 8, 2).and_hms(9, 30, 0));
                assert_eq!(pins, expected);
//
                repository.save_state(&PinOperation::new(
                    PinState::new(34, PinValue::Digital(false), Local.ymd(2019, 8, 2).and_hms(9, 0, 0), None),
                    "main".to_owned()
                ));

                assert!(state_retriever.all_zones_should_be_off(&nodes, &Local.ymd(2019, 8, 2).and_hms(9, 3, 0)));
                let pins = state_retriever.get_pins_expected_to_change(&nodes, &Local.ymd(2019, 8, 2).and_hms(9, 3, 0));
                assert_eq!(pins.len(), 0);

                assert!(state_retriever.all_zones_should_be_off(&nodes, &Local.ymd(2019, 8, 2).and_hms(9, 10, 1)));
                let expected: PinChanges = map!{ "main".to_owned() => map!{ 1 =>  PinValue::Analog(0) }};
                let pins = state_retriever.get_pins_expected_to_change(&nodes, &Local.ymd(2019, 8, 2).and_hms(9, 10, 1));
                assert_eq!(pins, expected);

                repository.save_state(&PinOperation::new(
                    PinState::new(1, PinValue::Analog(0), Local.ymd(2019, 8, 2).and_hms(9, 10, 0), None),
                    "main".to_owned()
                ));

                let expected: PinChanges = map!{ "main".to_owned() => map!{ 1 =>  PinValue::Analog(1023), 2 => PinValue::Analog(1023) }};
                let pins = state_retriever.get_pins_expected_to_change(&nodes, &Local.ymd(2019, 8, 2).and_hms(23, 2, 1));
                assert_eq!(pins, expected);

                let expected: PinChanges = map!{ "main".to_owned() => map!{ 1 =>  PinValue::Analog(1023), 2 => PinValue::Analog(1023) }};
                let pins = state_retriever.get_pins_expected_to_change(&nodes, &Local.ymd(2019, 8, 2).and_hms(23, 20, 1));
                assert_eq!(pins, expected);

                repository.save_state(&PinOperation::new(
                    PinState::new(1, PinValue::Analog(1023), Local.ymd(2019, 8, 2).and_hms(23, 2, 2), None),
                    "main".to_owned()
                ));
                repository.save_state(&PinOperation::new(
                    PinState::new(2, PinValue::Analog(1023), Local.ymd(2019, 8, 2).and_hms(23, 2, 2), None),
                    "main".to_owned()
                ));

                let expected: PinChanges = PinChanges::new();
                let pins = state_retriever.get_pins_expected_to_change(&nodes, &Local.ymd(2019, 8, 2).and_hms(23, 3, 1));
                assert_eq!(pins, expected);

                let expected: PinChanges = map!{ "main".to_owned() => map!{ 34 =>  PinValue::Digital(true) }};
                let pins = state_retriever.get_pins_expected_to_change(&nodes, &Local.ymd(2019, 8, 2).and_hms(23, 8, 2));
                assert_eq!(pins, expected);
            }
        }
    }
}