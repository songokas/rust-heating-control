use arduino_mqtt_pin::pin::{PinOperation, Temperature, PinCollection, PinState};
use std::collections::HashMap;
use crate::config::ControlNodes;
use chrono::{DateTime, Local};

pub type States = HashMap<String, HashMap<u8, PinCollection>>;

#[derive(new)]
pub struct PinStateRepository
{
    states: States
}

impl PinStateRepository
{
    pub fn save_state(&mut self, op: &PinOperation)
    {
        if self.states.contains_key(&op.node) {
            let result = self.states.get_mut(&op.node).map(|hmap| {
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
            self.states.insert(op.node.to_string(), col);
        }
    }

    pub fn get_last_changed_pin_state(&self, name: &str, pin: u8) -> Option<&PinState>
    {
        self.get_pin_collection(name, pin).and_then(|col: &PinCollection| col.get_last_changed())
    }

    pub fn get_average_temperature(&self, name: &str, pin: u8) -> Option<Temperature>
    {
        self.get_pin_collection(name ,pin).map(|col: &PinCollection| col.get_average_temperature())
    }

    pub fn get_first_zone_on_dt(&self, control_nodes: &ControlNodes, since: &DateTime<Local>) -> Option<DateTime<Local>>
    {
        control_nodes.iter().filter_map(|(control_name, control_node)|
            control_node.zones.iter().filter_map(|(zone_name, zone)|
                self.get_last_changed_pin_state(control_name, zone.control_pin).and_then(|state|
                    if state.is_on() && state.dt > *since { Some(state.dt) } else { None }
                )
            ).min()
        ).min().map(|dt| dt.clone())
    }

    fn get_pin_collection(&self, name: &str, pin: u8) -> Option<&PinCollection>
    {
        self.states.get(name).and_then(|nodes| nodes.get(&pin))
    }
}
