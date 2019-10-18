use chrono::{DateTime, Local, Duration};
use arduino_mqtt_pin::pin::{PinState, PinValue, Temperature};
use crate::config::{ControlNodes, Config};
use crate::repository::PinStateRepository;
use arduino_mqtt_pin::helper::percent_to_analog;
use crate::zone::Zone;

#[derive(new)]
pub struct ZoneStateDecider<'a>
{
    temp_decider: &'a TemperatureStateDecider<'a>,
    config: &'a Config
}

impl ZoneStateDecider<'_>
{

    pub fn should_be_on(&self, last_state: &PinState, zone: &Zone, current_temperature: &Temperature, now: DateTime<Local>) -> bool
    {
        if let Some(expected_temperature) = zone.get_expected_temperature(&now.time()) {
            if last_state.is_on() {
                *current_temperature < expected_temperature
            } else {
                *current_temperature < expected_temperature - Temperature::new(self.config.temperature_drop_wait)
            }
        } else {
            false
        }
    }

    pub fn get_value_to_change_to(&self, last_state: &PinState, zone: &Zone, current_temperature: &Temperature, now: &DateTime<Local>) -> Option<PinValue>
    {
        let zone_should_be_on = zone.get_expected_temperature(&now.time()).map(|expected_temperature|
            !last_state.is_on() && *current_temperature < expected_temperature - Temperature::new(self.config.temperature_drop_wait)
        ).unwrap_or(false);
        if last_state.is_on() && !zone_should_be_on {
            return Some(PinValue::Analog(0u16));
        }
        if !last_state.is_on() && zone_should_be_on {
            return self.temp_decider.get_expected_value(current_temperature, zone, now);
        }
        None
    }
}

#[derive(new)]
pub struct TemperatureStateDecider<'a>
{
    config: &'a Config
}

impl TemperatureStateDecider<'_>
{
    pub fn get_expected_value(&self, current_temperature: &Temperature, zone: &Zone, now: &DateTime<Local>) -> Option<PinValue>
    {
        let expected_temperature = zone.get_expected_temperature(&now.time())?;
        let diff = (expected_temperature - current_temperature.clone()).abs();
        let value = if diff <= Temperature::new(self.config.min_temperature_diff_for_pwm) {
            percent_to_analog(self.config.min_pwm_state)
        } else if (diff < Temperature { value: 1f32} ) {
            percent_to_analog((diff.value * 100f32) as u8)
        } else {
            percent_to_analog(100)
        };
        Some(PinValue::Analog(value))
    }
}

#[derive(new)]
pub struct HeaterDecider<'a>
{
    repository: &'a PinStateRepository,
    config: &'a Config
}

impl HeaterDecider<'_>
{
    pub fn should_be_on(&self, nodes: &ControlNodes, now: &DateTime<Local>) -> bool
    {
        if let Some(first_zone_on) = self.repository.get_first_zone_on_dt(nodes, &(*now - Duration::hours(24))) {
            return *now - first_zone_on > Duration::seconds(self.config.acctuator_warmup_time as i64);
        }
        false
    }

    pub fn can_turn_zones_off(&self, state: &PinState, now: &DateTime<Local>) -> bool
    {
        !state.is_on() && *now - state.dt > Duration::seconds(self.config.heater_pump_stop_time as i64)
    }
}

