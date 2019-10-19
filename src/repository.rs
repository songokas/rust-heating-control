use arduino_mqtt_pin::pin::{PinOperation, Temperature, PinCollection, PinState};
use std::collections::HashMap;
use crate::config::ControlNodes;
use chrono::{DateTime, Local};
use std::sync::RwLock;

pub type States = HashMap<String, HashMap<u8, PinCollection>>;

#[derive(new)]
pub struct PinStateRepository
{
    states: RwLock<States>
}

impl PinStateRepository
{
    pub fn save_state(&self, op: &PinOperation)
    {
        if self.states.read().unwrap().contains_key(&op.node) {
            let result = self.states.write().unwrap().get_mut(&op.node).map(|hmap| {
                hmap.get_mut(&op.pin_state.pin).map(|col| {
                    col.push(&op.pin_state);
                }).unwrap_or_else(|| {
                    let mut arr = PinCollection::default();
                    arr.push(&op.pin_state.clone());
                    hmap.insert(op.pin_state.pin, arr);
                });
            });
            if !result.is_some() {
                warn!("Failed to add state");
            }
        } else {
            let mut col = HashMap::new();
            let mut arr = PinCollection::default();
            arr.push(&op.pin_state.clone());
            col.insert(op.pin_state.pin, arr);
            self.states.write().unwrap().insert(op.node.to_string(), col);
        }
    }

    pub fn get_last_changed_pin_state(&self, name: &str, pin: u8) -> Option<PinState>
    {
        self.states.read().unwrap().get(name).and_then(|nodes| nodes.get(&pin))
            .and_then(|col: &PinCollection| col.get_last_changed())
    }

    pub fn get_average_temperature(&self, name: &str, pin: u8, since: &DateTime<Local>) -> Option<Temperature>
    {
        self.states.read().unwrap().get(name)
            .and_then(|nodes| nodes.get(&pin))
            .and_then(|col: &PinCollection| col.get_average_temperature(since))
    }

    pub fn get_first_zone_on_dt(&self, control_nodes: &ControlNodes, since: &DateTime<Local>) -> Option<DateTime<Local>>
    {
        control_nodes.iter().filter_map(|(control_name, control_node)| {
            control_node.zones.iter().filter_map(|(zone_name, zone)| {
                self.get_last_changed_pin_state(control_name, zone.control_pin).and_then(|state| {
                    state.is_on();
                    if state.is_on() && state.dt > *since { Some(state.dt) } else { None }
                })
            }).min()
        }).min().map(|dt| dt.clone())
    }

    // @TODO returns a value referencing data owned by the current function
    /*fn get_pin_collection(&self, name: &str, pin: u8) -> Option<&PinCollection>
    {
        self.states.read().unwrap().get(name).and_then(|nodes| nodes.get(&pin))
    }*/
}


#[cfg(test)]
pub mod test_repository
{
    use speculate::speculate;
    use super::*;
    use arduino_mqtt_pin::pin::PinValue;
    use chrono::{TimeZone, NaiveTime};
    use crate::zone::{Zone, Interval};
    use crate::config::{ControlNode};

    pub fn create_repository() -> PinStateRepository
    {
        let pin_one_states = vec![
            PinState::new(1, PinValue::Analog(0), Local.ymd(2019, 8, 1).and_hms(8, 0, 0), None),
            PinState::new(1, PinValue::Analog(255), Local.ymd(2019, 8, 2).and_hms(8, 0, 0), None),
        ];
        let temperature_one_states = vec![
            PinState::new(4, PinValue::Temperature(Temperature::new(18.5)), Local.ymd(2019, 8, 2).and_hms(7, 40, 0), None),
            PinState::new(4, PinValue::Temperature(Temperature::new(17.0)), Local.ymd(2019, 8, 2).and_hms(8, 3, 0), None),
            PinState::new(4, PinValue::Temperature(Temperature::new(18.5)), Local.ymd(2019, 8, 2).and_hms(8, 30, 0), None),
            PinState::new(4, PinValue::Temperature(Temperature::new(19.5)), Local.ymd(2019, 8, 2).and_hms(8, 50, 0), None),
        ];
        let pin_two_states = vec![
            PinState::new(2, PinValue::Analog(122), Local.ymd(2019, 8, 1).and_hms(8, 0, 0), None),
            PinState::new(2, PinValue::Analog(0), Local.ymd(2019, 8, 1).and_hms(9, 0, 0), None),
        ];
        let temperature_two_states = vec![
            PinState::new(4, PinValue::Temperature(Temperature::new(20.5)), Local.ymd(2019, 8, 1).and_hms(8, 50, 0), None),
        ];
        let pin_five_states = vec![
            PinState::new(5, PinValue::Analog(0), Local.ymd(2019, 8, 2).and_hms(8, 5, 0), None),
        ];
        let temperature_five_states = vec![
            PinState::new(4, PinValue::Temperature(Temperature::new(22.5)), Local.ymd(2019, 8, 2).and_hms(8, 0, 0), None),
        ];
        let pin_heater_states = vec![
            PinState::new(34, PinValue::Digital(true), Local.ymd(2019, 8, 2).and_hms(8, 10, 0), None),
        ];
        let states = map!{
            "main".to_owned() => map!{
                1 => PinCollection::from_states(&pin_one_states),
                2 => PinCollection::from_states(&pin_two_states),
                5 => PinCollection::from_states(&pin_five_states),
                34 => PinCollection::from_states(&pin_heater_states)
            },
            "zone1".to_owned() => map!{
                4 => PinCollection::from_states(&temperature_one_states)
            },
            "zone2".to_owned() => map!{
                4 => PinCollection::from_states(&temperature_two_states)
            },
            "zone4".to_owned() => map!{
               4 => PinCollection::from_states(&temperature_five_states)
            },
            "zone5".to_owned() => map!{
               4 => PinCollection::from_states(&temperature_five_states)
            }
        };
        PinStateRepository::new(RwLock::new(states))
    }

    pub fn create_zone(control_pin: u8) -> Zone
    {
        let intervals = vec![
            Interval::new(NaiveTime::from_hms(8, 0, 0), NaiveTime::from_hms(9, 0, 0), Temperature::new(20.0)),
            Interval::new(NaiveTime::from_hms(23, 1, 0), NaiveTime::from_hms(23, 31, 0), Temperature::new(30.5))
        ];
        Zone::new(format!("zone{}", control_pin), 4, intervals, control_pin)
    }

    pub fn create_nodes() -> ControlNodes
    {
        let nodes = map!{"main".to_owned() => ControlNode::new(
            "main".to_owned(), 34, map!{
                "zone1".to_owned() => create_zone(1),
                "zone2".to_owned() => create_zone(2),
                "zone4".to_owned() => create_zone(4)
            }
        )};
        nodes
    }

    speculate!{
        describe "state repository"
        {
            before
            {
                let repository = create_repository();
            }

            it "should get last pin state"
            {
                for (name, expected, pin) in vec![
                    ("exists 1", true, 1),
                    ("exists 2", true, 2),
                    ("does not exists", false, 3),
                ] {
                    assert_eq!(
                        repository.get_last_changed_pin_state("main", pin).is_some(),
                        expected,
                        "{}", name
                    );
                }


                for (expected, pin) in vec![
                    (255, 1),
                    ( 0, 2),
                ] {
                    assert_eq!(
                        repository.get_last_changed_pin_state("main", pin).unwrap().value,
                        PinValue::Analog(expected),
                        "should be: {}", expected
                    );
                }
            }

            it "should get average temperature"
            {

                let since = Local.ymd(2019, 8, 1).and_hms(7, 0, 0);

                assert_eq!(
                    repository.get_average_temperature("zone1", 4, &since).expect("zone 1 temp"),
                    Temperature::new(18.375)
                );

                assert!(
                    repository.get_average_temperature("main", 34, &since).is_none()
                );

                assert_eq!(
                    repository.get_average_temperature("zone4", 4, &since).expect("zone 5 temp"),
                    Temperature::new(22.5)
                );

            }

            it "should get last dt on"
            {
                let nodes = create_nodes();
                let since = Local.ymd(2019, 8, 1).and_hms(7, 0, 0);
                let expected = Local.ymd(2019, 8, 2).and_hms(8, 0, 0);

                assert_eq!(
                    repository.get_first_zone_on_dt(&nodes, &since),
                    Some(expected)
                );

                let since = Local.ymd(2019, 9, 1).and_hms(7, 0, 0);
                assert_eq!(
                    repository.get_first_zone_on_dt(&nodes, &since),
                    None
                );
            }
        }
    }
}