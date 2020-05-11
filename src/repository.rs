use arduino_mqtt_pin::pin::{PinOperation, Temperature, PinCollection, PinState, PinValue};
use std::collections::HashMap;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use diesel::{insert_into, RunQueryDsl, SqliteConnection};
use diesel::prelude::*;
use arduino_mqtt_pin::helper::average;

use crate::config::ControlNodes;
use crate::schema::pin_states;
use crate::schema::temperatures;
use crate::schema::pin_states::BoxedQuery;
use diesel::query_dsl::QueryDsl;
use uuid::Uuid;
use diesel::sqlite::{Sqlite};
use derive_new::{new};

pub type States = HashMap<String, HashMap<u8, PinCollection>>;

#[derive(new, Insertable, Queryable, Identifiable, Debug, PartialEq)]
#[table_name = "pin_states"]
struct PinRow
{
    id: String,
    name: String,
    pin: i32,
    input_type: i32,
    value: i32,
    dtc: NaiveDateTime
}
//
#[derive(new, Insertable, Queryable, Identifiable, Debug, PartialEq)]
#[table_name = "temperatures"]
struct PinTemperature
{
    id: String,
    name: String,
    pin: i32,
    temperature: f32,
    dtc: NaiveDateTime
}

#[derive(new)]
pub struct PinStateRepository<'a>
{
    conn: &'a SqliteConnection
}

struct PinStateBuilder
{
    builder: BoxedQuery<'static, Sqlite>
}

impl PinStateBuilder
{
    pub fn from_name(name_id: &str, pin_id: u8) -> PinStateBuilder
    {
        use crate::schema::pin_states::dsl::{name, pin, pin_states};
        let builder: BoxedQuery<'static, Sqlite> = pin_states.filter(name.eq(name_id.to_owned()))
            .filter(pin.eq(pin_id as i32)).into_boxed();
        PinStateBuilder { builder }
    }

    pub fn with_value(self, state: bool) -> PinStateBuilder
    {
        use crate::schema::pin_states::dsl::{value};
        if state {
            PinStateBuilder { builder: self.builder.filter(value.gt(0)) }
        } else {
            PinStateBuilder { builder: self.builder.filter(value.eq(0)) }
        }
    }

    pub fn with_less(self, time: &NaiveDateTime) -> PinStateBuilder
    {
        use crate::schema::pin_states::dsl::{dtc};
        PinStateBuilder { builder: self.builder.filter(dtc.lt(time.clone())) }
    }

    pub fn with_less_or_eq(self, time: &NaiveDateTime) -> PinStateBuilder
    {
        use crate::schema::pin_states::dsl::{dtc};
        PinStateBuilder { builder: self.builder.filter(dtc.le(time.clone())) }
    }

    pub fn within_period(self, from: &NaiveDateTime, to: &NaiveDateTime) -> PinStateBuilder
    {
        use crate::schema::pin_states::dsl::{dtc};
        PinStateBuilder {
            builder: self.builder
                .filter(dtc.le(to.clone()))
                .filter(dtc.gt(from.clone()))
        }
    }

    pub fn order_by_time(self, desc: bool) -> PinStateBuilder
    {
        use crate::schema::pin_states::dsl::{dtc};
        if desc {
            PinStateBuilder { builder: self.builder.order(dtc.desc() ) }
        } else {
            PinStateBuilder { builder: self.builder.order(dtc.asc() ) }
        }
    }

    pub fn first_to_state(self, conn: &SqliteConnection) -> Option<PinState>
    {
        self.builder.limit(1).load::<PinRow>(conn).ok()
            .and_then(|arr| {
                arr.first().and_then(|row| {
                    Some(PinState::new(
                        row.pin as u8,
                        match row.input_type {
                            0 => PinValue::Digital(row.value > 0),
                            _ => PinValue::Analog(row.value as u16)
                        },
                        Local.from_local_datetime(&row.dtc).single()?,
                        None
                    ))
                })
            })
    }
}

impl PinStateRepository<'_>
{
    pub fn save_state(&self, op: &PinOperation)
    {
        use crate::schema::temperatures::dsl::{temperatures};
        use crate::schema::pin_states::dsl::{pin_states};

        if let PinValue::Temperature(temp) = &op.pin_state.value {
            let temp = PinTemperature::new(format!("{}", Uuid::new_v4()), op.node.clone(), op.pin_state.pin as i32, temp.value, op.pin_state.dt.naive_local());
            insert_into(temperatures).values(&temp).execute(self.conn).unwrap();
        } else {
            let pin = PinRow::new(
                format!("{}", Uuid::new_v4()),
                op.node.clone(),
                op.pin_state.pin as i32,
                match op.pin_state.value { PinValue::Digital(v) => 0, _ => 1 },
                op.pin_state.value.as_u16() as i32,
                op.pin_state.dt.naive_local()
            );
            insert_into(pin_states).values(&pin).execute(self.conn).unwrap();
        }
    }

    pub fn get_last_changed_pin_state(&self, name_id: &str, pin_id: u8) -> Option<PinState>
    {
        self.get_pin_changes(name_id, pin_id, 1).and_then(|arr| arr.first().cloned() )
    }

    pub fn get_last_pin_state(&self, name_id: &str, pin_id: u8) -> Option<PinState>
    {
        PinStateBuilder::from_name(name_id, pin_id).order_by_time(true).first_to_state(self.conn)
    }

    // node1 0 12:32:36 -> returns
    // node1 1 12:32:35 -> returns
    // node1 0 12:32:34
    // node1 0 12:32:33 -> returns
    // node2 1 12:32:38
    // node3 1 12:32:37 -> returns
    pub fn get_pin_changes(&self, name_id: &str, pin_id: u8, how_many: usize) -> Option<Vec<PinState>>
    {
        if let Some(changed_dates) = self.get_latest_pin_change_dates(name_id, pin_id, how_many + 1) {
            let mut it = changed_dates.into_iter();
            let mut state_arr = Vec::new();
            let mut i = 0;
            let mut latest_state = it.next();
            while i < how_many {
                if let Some((latest_value, latest_dtc)) = latest_state {
                    if let Some((previous_value, previous_dtc)) = it.next() {
                        if let Some(state) = PinStateBuilder::from_name(name_id, pin_id)
                                .with_value(latest_value)
                                .order_by_time(false)
                                .within_period(&previous_dtc, &latest_dtc)
                                .first_to_state(self.conn) {
                            state_arr.push(state);
                        }
                        latest_state = Some((previous_value, previous_dtc));
                    } else {
                        if let Some(state) = PinStateBuilder::from_name(name_id, pin_id)
                                .with_value(latest_value)
                                .order_by_time(false)
                                .with_less_or_eq(&latest_dtc)
                                .first_to_state(self.conn) {
                            state_arr.push(state);
                        }
                        latest_state = None;
                    }
                } else {
                    break;
                }
                i += 1;
            }
            return Some(state_arr);
        }
        None
    }

    pub fn get_average_temperature(&self, name_id: &str, pin_id: u8, since: &DateTime<Local>) -> Option<Temperature>
    {
        use crate::schema::temperatures::dsl::*;
        let values = temperatures.filter(pin.eq(pin_id as i32))
            .filter(dtc.ge(since.naive_local()))
            .filter(name.eq(name_id))
            .select(temperature)
            .load::<f32>(self.conn);
        values.ok().and_then(|v| if v.len() > 0 { Some(v) } else { None }).map(|v| Temperature::new(average(&v)))
    }

    // node1 1 333 12:32:32
    // node1 2 333 12:32:33
    // node1 2 0 12:32:34
    // node2 1 0 12:32:35
    // node3 3 0 12:32:36
    // node1 1 222 12:32:37
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

    // returns only pairs
    // node1 1 12:32:33 -> returns
    // node1 1 12:32:32
    // node1 0 12:32:36 -> returns
    // node1 0 12:32:33
    // node1 1 12:32:37 -> returns
    fn get_latest_pin_change_dates(&self, name_id: &str, pin_id: u8, size: usize) -> Option<Vec<(bool, NaiveDateTime)>>
    {
        if let Some(state) = self.get_last_pin_state(name_id, pin_id) {
            let mut arr = vec![(state.is_on(), state.dt.naive_local())];
            let mut state_index = state;
            while arr.len() < size {
                if let Some(state) = PinStateBuilder::from_name(name_id, pin_id)
                        .with_value(!state_index.is_on())
                        .order_by_time(true)
                        .with_less(&state_index.dt.naive_local())
                        .first_to_state(self.conn) {
                    arr.push((state.value.is_on(), state.dt.naive_local()));
                    state_index = state;
                } else {
                    break;
                }
            }
            return Some(arr);
        }
        None
    }
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
    use crate::embedded_migrations;

    pub fn get_data() -> HashMap<String, HashMap<u8, Vec<PinState>>>
    {

        let pin_one_states = vec![
            PinState::new(1, PinValue::Analog(0), Local.ymd(2019, 8, 1).and_hms(8, 0, 0), None),
            PinState::new(1, PinValue::Analog(0), Local.ymd(2019, 8, 1).and_hms(8, 1, 0), None),
            PinState::new(1, PinValue::Analog(0), Local.ymd(2019, 8, 1).and_hms(8, 2, 0), None),
            PinState::new(1, PinValue::Analog(255), Local.ymd(2019, 8, 2).and_hms(8, 0, 0), None),
        ];
        let temperature_one_states = vec![
            PinState::new(4, PinValue::Temperature(Temperature::new(18.5)), Local.ymd(2019, 8, 2).and_hms(7, 40, 0), None),
            PinState::new(4, PinValue::Temperature(Temperature::new(17.0)), Local.ymd(2019, 8, 2).and_hms(8, 3, 0), None),
            PinState::new(4, PinValue::Temperature(Temperature::new(18.5)), Local.ymd(2019, 8, 2).and_hms(8, 30, 0), None),
            PinState::new(4, PinValue::Temperature(Temperature::new(19.5)), Local.ymd(2019, 8, 2).and_hms(8, 50, 0), None),
            PinState::new(4, PinValue::Temperature(Temperature::new(19.5)), Local.ymd(2019, 8, 2).and_hms(8, 55, 0), None),
        ];
        let pin_two_states = vec![
            PinState::new(2, PinValue::Analog(122), Local.ymd(2019, 8, 1).and_hms(8, 0, 0), None),
            PinState::new(2, PinValue::Analog(0), Local.ymd(2019, 8, 1).and_hms(9, 0, 0), None),
            PinState::new(2, PinValue::Analog(0), Local.ymd(2019, 8, 1).and_hms(10, 0, 0), None),
            PinState::new(2, PinValue::Analog(0), Local.ymd(2019, 8, 1).and_hms(11, 0, 0), None),
        ];
        let temperature_two_states = vec![
            PinState::new(4, PinValue::Temperature(Temperature::new(20.5)), Local.ymd(2019, 8, 1).and_hms(8, 50, 0), None),
        ];
        let pin_five_states = vec![
            PinState::new(5, PinValue::Analog(0), Local.ymd(2019, 8, 2).and_hms(8, 5, 0), None),
        ];
        let temperature_five_states = vec![
            PinState::new(4, PinValue::Temperature(Temperature::new(22.5)), Local.ymd(2019, 8, 2).and_hms(8, 30, 0), None),
        ];
        let pin_heater_states = vec![
            PinState::new(34, PinValue::Digital(true), Local.ymd(2019, 8, 2).and_hms(8, 10, 0), None),
            PinState::new(34, PinValue::Digital(true), Local.ymd(2019, 8, 2).and_hms(8, 11, 0), None),
            PinState::new(34, PinValue::Digital(true), Local.ymd(2019, 8, 2).and_hms(8, 12, 0), None),
        ];
        let pin_eight_states = vec![
            PinState::new(8, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 5, 0), None),
            PinState::new(8, PinValue::Analog(0), Local.ymd(2018, 8, 2).and_hms(8, 6, 0), None),
            PinState::new(8, PinValue::Analog(0), Local.ymd(2018, 8, 2).and_hms(8, 7, 0), None),
            PinState::new(8, PinValue::Analog(0), Local.ymd(2018, 8, 2).and_hms(8, 8, 0), None),
            PinState::new(8, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 9, 0), None),
            PinState::new(8, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 9, 1), None),
            PinState::new(8, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 9, 2), None),
            PinState::new(8, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 9, 3), None),
            PinState::new(8, PinValue::Analog(0), Local.ymd(2018, 8, 2).and_hms(8, 10, 0), None),
            PinState::new(8, PinValue::Analog(0), Local.ymd(2018, 8, 2).and_hms(8, 11, 0), None),
        ];

        let pin_nine_states = vec![
            PinState::new(9, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 5, 0), None),
            PinState::new(9, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 6, 0), None),
            PinState::new(9, PinValue::Analog(0), Local.ymd(2018, 8, 2).and_hms(8, 7, 0), None),
            PinState::new(9, PinValue::Analog(0), Local.ymd(2018, 8, 2).and_hms(8, 8, 0), None),
            PinState::new(9, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 9, 0), None),
            PinState::new(9, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 9, 1), None),
            PinState::new(9, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 9, 2), None),
            PinState::new(9, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 9, 3), None),
            PinState::new(9, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 10, 0), None),
            PinState::new(9, PinValue::Analog(1), Local.ymd(2018, 8, 2).and_hms(8, 11, 0), None),
        ];

        let data = map!{
            "main".to_owned() => map!{
                1 => pin_one_states,
                2 => pin_two_states,
                5 => pin_five_states,
                8 => pin_eight_states,
                9 => pin_nine_states,
                34 => pin_heater_states
            },
            "zone1".to_owned() => map!{
                4 => temperature_one_states
            },
            "zone2".to_owned() => map!{
                4 => temperature_two_states
            },
            "zone4".to_owned() => map!{
               4 => temperature_five_states
            }
        };
        data
    }

    pub fn create_repository(conn: &SqliteConnection) -> PinStateRepository
    {

        let data = get_data();
        let repo = PinStateRepository::new(conn);
        for (op_name, pin_states) in data {
            for (pin, states) in pin_states {
                for state in states {
                    repo.save_state(&PinOperation::new(state, op_name.clone()));
                }
            }
        }
        repo
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

                let connection = SqliteConnection::establish(":memory:").unwrap();
                embedded_migrations::run(&connection);
                let repository = create_repository(&connection);
                let data = get_data();
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

            it "should get last pins"
            {
                for (zone, pin, get_len, expected_indexes) in vec![
                    ("main", 8, 5, vec![8, 4, 1, 0]),
                    ("main", 9, 2, vec![4, 2]),
                ] {
                    assert_eq!(
                        repository.get_pin_changes(zone, pin, get_len).unwrap()[..],
                        data.get(zone).and_then(|m| m.get(&pin)).map(|data_array| expected_indexes.iter().map(|i| data_array[*i].clone()).collect::<Vec<PinState>>()).unwrap()[..],
                    );
                }

            }

            it "should get average temperature"
            {

                let since = Local.ymd(2019, 8, 2).and_hms(8, 30, 0);

                assert_eq!(
                    repository.get_average_temperature("zone1", 4, &since).expect("zone 1 temp"),
                    Temperature::new(19.166666)
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
