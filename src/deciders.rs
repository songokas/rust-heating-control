use chrono::{DateTime, Local, Duration};

#[derive(new)]
struct ZoneStateDecider<'t, 'c>
{
    temp_decider: &'t TemperatureStateDecider,
    config: &'c Config
}

impl ZoneStateDecider
{

    pub fn should_be_on(last_state: &PinState, zone: &Zone, current_temperature: &Temperature, now: DateTime<Local>) -> bool
    {
        if let Some(expected_temperature) = zone.get_exppected_temperature(now) {
            if last_state.is_on() {
                current_temperature < expected_temperature
            } else {
                current_temperature < expected_temperature - Temperature::new(config.temperature_drop_wait)
            }
        } else {
            false
        }
    }

    pub fn get_value_to_change_to(last_state: &PinState, zone: &Zone, current_temperature: &Temperature, now: DateTime<Local>) -> Option<PinValue>
    {
        let zone_should_be_on = if let Some(expected_temperature) = zone.get_exppected_temperature(now) && !last_state.is_on() {
            current_temperature < expected_temperature - Temperature::new(config.temperature_drop_wait)
        } else {
            false
        };
        if last_state.is_on() && !zone_should_be_on {
            return Some(PinValue::Analog(0u16));
        }
        if !last_state.is_on() && zone_should_be_on {
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
    pub fn should_be_on(&self, nodes: &ControlNodes, now: &DateTime<Local>s) -> bool
    {
        if Some(firt_zone_on) = repository.get_first_zone_on(nodes, now - Duration::hours(24)) {
            return now - first_zone_on > Duration::seconds(config.acctuator_warmup_time as i64);
        }
        false
    }

    pub fn can_turn_zones_off(&self, state: &PinState, now: &DateTime<Local>)
    {
        state.is_off() && now - state.dt > Duration::seconds(config.heater_pump_stop_time as i64) 
    }
}

