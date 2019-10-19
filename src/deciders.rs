use chrono::{DateTime, Local, Duration};
use arduino_mqtt_pin::pin::{PinState, PinValue, Temperature};
use crate::config::{ControlNodes, Config};
use crate::repository::PinStateRepository;
use arduino_mqtt_pin::helper::percent_to_analog;
use crate::zone::{Interval, Zone}       ;

#[derive(new)]
pub struct ZoneStateDecider<'a>
{
    temp_decider: &'a TemperatureStateDecider<'a>,
    config: &'a Config
}

impl ZoneStateDecider<'_>
{

    pub fn should_be_on(&self, last_state: &PinState, zone: &Zone, current_temperature: &Temperature, now: &DateTime<Local>) -> bool
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
        let zone_should_be_on = self.should_be_on(last_state, zone, current_temperature, now);
        if last_state.is_on() && !zone_should_be_on {
            return Some(PinValue::Analog(0u16));
        }
        if !last_state.is_on() && zone_should_be_on {
            return Some(self.temp_decider.get_expected_value(current_temperature, zone, now));
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
    pub fn get_expected_value(&self, current_temperature: &Temperature, zone: &Zone, now: &DateTime<Local>) -> PinValue
    {
        let expected_temperature = zone.get_expected_temperature(&now.time());
        if expected_temperature.is_none() {
            return PinValue::Analog(0);
        }
        let expected_temperature = expected_temperature.unwrap();
        if *current_temperature >= expected_temperature {
            return PinValue::Analog(0);
        }
        let diff = (expected_temperature - current_temperature.clone()).abs();
        let value = if diff <= Temperature::new(self.config.min_temperature_diff_for_pwm) {
            percent_to_analog(self.config.min_pwm_state)
        } else if diff < Temperature::new(1f32) {
            percent_to_analog((diff.value * 100f32) as u8)
        } else {
            percent_to_analog(100)
        };
        PinValue::Analog(value)
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


#[cfg(test)]
mod tests {

    use super::*;
    use chrono::{TimeZone, NaiveTime};

    fn create_zone() -> (Zone, Config)
    {
        let config = Config::new(String::from("test"), String::from("host"), String::from("main"), 3);
        let intervals = vec![
            Interval::new(NaiveTime::from_hms(8, 0, 0), NaiveTime::from_hms(9, 0, 0), Temperature::new(20.0)),
            Interval::new(NaiveTime::from_hms(23, 1, 0), NaiveTime::from_hms(23, 3, 3), Temperature::new(30.5))
        ];
        (Zone::new(String::from("zone1"), 1, intervals, 2), config)
    }

    speculate! {
        describe "zone temperature"
        {
            before
            {
                let (zone, config) = create_zone();
                let decider = TemperatureStateDecider::new(&config);
            }

            it "should match value"
            {
                for (expected, temp, hour) in vec![
                    (1023, 19.0, 8),
                    (306, 19.8, 8),
                    (716, 19.3, 8),
                    (1023, 2.3, 8),
                    (0, 20.0, 8),
                    (0, 20.0, 9),
                ] {
                    assert_eq!(
                        decider.get_expected_value(
                            &Temperature::new(temp), &zone, &Local.ymd(2019, 8, 1).and_hms(hour, 0, 0)
                        ).as_u16(),
                        expected
                    );
                }
            }
        }

        describe "zone states"
        {
            before
            {
                let (zone, config) = create_zone();
                let temp_decider = TemperatureStateDecider::new(&config);
                let zone_decider = ZoneStateDecider::new(&temp_decider, &config);
            }

            it "should provide zone state"
            {
                for (expected, value, temp, hour) in vec![
                    (true, 1023, 19.0, 8),
                    (true, 0, 19.0, 8),
                    (true, 306, 19.8, 8),
                    (true, 716, 19.3, 8),
                    (true, 0, 19.25, 8),
                    (false, 0, 19.4, 8),
                    (false, 716, 20.0, 8),
                    (false, 716, 19.0, 9),
                    (false, 0, 19.0, 9),
                ] {
                    assert_eq!(
                        zone_decider.should_be_on(
                            &PinState::new(1, PinValue::Analog(value), Local.ymd(2019, 8, 1).and_hms(8, 0, 0), None),
                            &zone,
                            &Temperature::new(temp),
                            &Local.ymd(2019, 8, 1).and_hms(hour, 0, 0)
                        ),
                        expected,
                        "test with {} {}", value, temp
                    );
                }
            }
        }
    }
}

