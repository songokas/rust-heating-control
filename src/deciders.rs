use chrono::{DateTime, Local, Duration};

#[derive(new)]
struct ZoneStateDecider<'t, 'c>
{
    temp_decider: &'t TemperatureStateDecider,
    config: &'c Config
}

impl ZoneStateDecider
{
    pub fn get_value_to_change_to(last_state: &PinState, zone: &Zone, current_temperature: &Temperature, now: DateTime<Local>) -> Option<PinValue>
    {
        let zone_should_be_on = if let Some(expected_temperature) = zone.get_exppected_temperature(now) && !last_state.is_on() {
            current_temperature < expected_temperature - Temperature::new(config.temperature_drop_wait)
        } else {
            false
        };
        if last_state.is_on && !zone_should_be_on {
            return Some(PinValue::Analog(0u16));
        }
        if !last_state.is_on && zone_should_be_on {
            return current_temperature.map(|temperature| temp_decider.get_expected_value(temperature, zone)).unwrap_or(0);
        }
        None
    }
}

#[derive(new)]
struct TemperatureStateDecider<'c>
{
    config: &'c Config
}

impl TemperatureStateDecider
{
    pub fn get_expected_value(&self, current_temperature: &Temperature, zone: &Zone, now: &DateTime<Local>) -> Option<PinValue>
    {
        let expected_temperature = zone.get_expected_temperature(now)?;
        let diff = (expected_temperature - current_temperature).abs();
        let value = if diff <= Temperature::new(config.min_temperature_diff_for_pwm) {
            percent_to_analog(config.min_pwm_state)
        } else if (diff < Temperature { value: 1f32} ) {
            percent_to_analog((diff.value * 100f32) as u8)
        } else {
            percent_to_analog(100)
        };
        PinValue::Analog(value)
    }
}

#[derive(new)]
struct HeaterDecider<'c, 'r>
{
    repository: &'r PinStateRepository
    config: &'c Config
}

impl HeaterDecider
{
    pub fn should_be_on(&self, now: &DateTime<Local>, pins:[&u8]) -> bool
    {
        if Some(firt_zone_on) = repository.get_first_zone_on(now, pins) {
            return now - first_zone_on > Duration::seconds(config.acctuator_warmup_time as i64);
        }
        false
    }

    pub fn should_be_off(
        &self,
        state: Option<&PinState>,
        control_values: &PinChanges,
        now: &DateTime<Local>
    ) -> bool
    {

        if Some(zones) = repository.get_zones_on(now, ) {
            return zones.len() == 0;
        }
        false
    }

    /*pub fn get_value_to_change_to(&self, first_zone_on: &DateTime<Local>, now: &DateTime<Local>) -> Option<PinValue>
    {
        let can_start = Local::now() - first_zone_on > chrono::Duration::seconds(config.acctuator_warmup_time as i64);
    }

    pub fn get_changes(
        &self,
        state: Option<&PinState>,
        first_zone_on: Option<&DateTime<Local>,
        control_values: &PinChanges,
        now: &DateTime<Local>
    ) -> HashMap<u8, PinValue>
    {
        let mut controls: Zones = Hash::new();
        if let Some(current_state)  = state {
            if current_state.is_on() {
                repository.are_all_states_off
            }
            if current_state.is_off() {
                let can_start = '';

                self.get_pin_collection(name, pin).map(|col| col.get_(duration)))
            } else if current_state.is_on()
                if expected_value.is_on() && !current_state.is_on() {
                    controls.insert(current_state.pin, expected_value);
                } else if !expected_value.is_on() && current_state.is_on() {
                    controls.insert(current_state.pin, expected_value);
                }
            }

        let currently_turning_off = changes.get(state.pin).map(|value| !value.is_on()).unwrap_or(false);
        let is_shutdown_period = state.map(|current_state| Local::now() - Duration::seconds(config.heater_pump_stop_time) < current_state.dt).unwrap_or(false);

        let mut changes: PinChanges = HashMap::new();
        if controls.len() > 0 {
            changes.insert(state.pin, controls);
        }

        for (control_name, pin_changes) in control_values {
            let mut zone_values: Zones = HashMap::new();
            for (pin, value) in pin_changes {
                if curently_turning_off || is_shutdown_period {
                    if value.is_on(){
                        zone_values.insert(pin, value);
                    }
                } else {
                    zone_values.insert(pin, value);
                }
            }
            if (zone_values.len > 0) {
                changes.insert(control_name, zone_values);
            }
        }
        changes
    }*/
}

