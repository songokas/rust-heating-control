table! {
    pin_states (id) {
        id -> Text,
        name -> Text,
        pin -> Integer,
        input_type -> Integer,
        value -> Integer,
        dtc -> Timestamp,
    }
}

table! {
    temperatures (id) {
        id -> Text,
        name -> Text,
        pin -> Integer,
        temperature -> Float,
        dtc -> Timestamp,
    }
}

allow_tables_to_appear_in_same_query!(
    pin_states,
    temperatures,
);
