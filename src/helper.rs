
pub fn send_to_zone(client: &Mosquitto, pin: u8, value: u16, namespace: &str, name: &str) -> bool
{
    let data = object!{
        "pin" => pin,
        "set" => value
    };

    let topic = format!("{namespace}/nodes/{name}/set/json", namespace=namespace, name=name);

    let result = client.publish(
        &topic,
        data.dump().as_bytes(),
        1,
        true
    );

    debug!("Message sent: {} {}", topic, data.dump());

    if let Err(v) = result {
        warn!("Unable to send data to {}", name);
        return false;
    }
    true
}

pub fn print_info(control_nodes: &ControlNodes, repository: &StatePinRepository)
{
    for (control_name, node) in control_nodes {
        for (zone_name, zone) in node.zones {
            let state = repository.get_last_changed_pin_state(control_name, zone.control_pin);
            let temp = repository.get_average_temperature(zone_name, zone.sensor_pin);
            debug!("Node: {} State: {:?}", control_name, state);
            debug!("Zone: {} Temperature: {}", zone_name, temp);
        }
    }
}



