type PinChanges = HashMap<String, HashMap<u8, PinValue>>;

#[derive(Default)]
struct StateRetriever<'r, 'h, 'z, 'c>
{
    repository: &'r PinStateRepository,
    heater_decider: &'h HeaterDecider,
    zone_decider: &'z ZoneDecider,
    config: &'c Config
}

impl StateRetriever
{
    pub fn get_zone_pins_to_change(&self, control_name: &str, zones: &Zones)
    {
        let mut zone_changes: HashMap<u8, PinValue> = HashMap::new();
        for (zone_name, zone) in zones {
            if let Some(last_state) = repository.get_last_changed_pin_state(control_name, zone.control_pin) {
                if let Some(avg_temp) = repository.get_average_temperature(zone_name, zone.sensor_pin, Duration::seconds(config.consider_duration)) {
                    zone_decider.get_value_to_change_to(last_state, zone, avg_tem)
                        .map(|value| zone_changes.insert(zone.contro_pin, value));
                } else if last_state.is_on() {
                    zone_changes.insert(zone.control_pin, PinValue::Analog(0u16));
                }
            }
        }
        zone_changes
    }

    pub fn get_pins_expected_to_change(&self, control_nodes: &ControlNodes) -> PinChanges
    {
        let now = Local::now();
        let mut control_changes: PinChanges = PinChanges::new();


        let turn_heater = |control_changes, value | {
            control_changes.entry(config.heater_control_name).and_modify(|zone_changes| {
                zone_changes.insert(config.heater_control_pin, PinValue::Digital(value));
            }).or_insert_with(|| {
                let zone_changes = ZoneChanges::new();
                zone_changes.insert(config.heater_control_pin, PinValue::Digital(value));
            });
        };

        let current_state = repository.get_last_changed_pin_state(config.heater_control_name, config.heater_control_pin);
            if state.is_on() {
                let pins = pins.append(control_changes.iter());
                if heater_decider.should_be_off(pins, now) {
                    // we can not change
                    conrol_changes.clear();
                    turn_heater(control_changes, false);
                    return control_changes;
                }
            } else {
                if !heater_decider.can_turn_zones_off(state) {
                    return control_changes;
                }
                if heater_decider.should_be_on(pins, now) {
                    turn_heater(true);
                }
            }
        } else if heater_decider.should_be_on(pins) {
            turn_heater(true);
        }

        for (control_name, control_node) in control_nodes {
            let zone_changes = self.get_zone_pins_to_change(control_name, control_node.zones);
            control_changes.insert(control_name, zone_changes);
        }




        control_changes
        /*firt_zone_on
        heater_decider.get_changes(
            repository.get_last_changed_pin_state(config.heater_control_name, config.heater_control_pin),
            repository.get_first_zone_on_dt(dt),
            control_changes,
            now
        );*/
    }

    pub fn get_heater_pins_expected_to_chagn
}
