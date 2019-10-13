
pub type States = HashMap<String, HashMap<u8, PinCollection>>;

#[derive(Default)]
struct PinStateRepository<'s>
{
    states: &'s States
}

impl PinStateRepository
{
    /*pub fn new() -> PinStateRepository
    {
        PinStateRepository { states: States::new() }
    }
*/
    pub fn save_state(&mut self, op: &PinOperation)
    {
        if states.contains_key(&op.node) {
            let result = states.get_mut(&op.node).map(|hmap| {
                hmap.get_mut(&op.pin_state.pin).map(|col| {
                    col.push(&op.pin_state);
                }).unwrap_or_else(|| {
                    let mut arr = PinCollection::new();
                    arr.push(&op.pin_state.clone());
                    hmap.insert(op.pin_state.pin, arr);
                });
            });
            if !result.is_some() {
                warn!("Failed to add state");
            }
        } else {
            let mut col = HashMap::new();
            let mut arr = PinCollection::new();
            arr.push(&op.pin_state.clone());
            col.insert(op.pin_state.pin, arr);
            states.insert(op.node.to_string(), col);
        }
    }

    pub fn get_last_changed_pin_state(&self, name: &str, pin: u8) -> Option<PinState>
    {
        self.get_pin_collection(name, pin).map(|col: &PinCollection| col.get_last_changed())
    }

    pub fn get_average_temperature(&self, name: &str, pin: u8, duration: &Duration) -> Option<Temperature>
    {
        self.get_pin_collection(name ,pin).map(|col: &PinCollection| col.get_average_temperature(duration))
    }

    pub fn get_first_zone_on_dt(&self, duration: &Duration) -> Option<PinState>
    {
        let first_by_node = self.states.iter().map(|pins| {
            pins.iter().filter_map(|col| if col.is_on() { Some(col.get_last_changed_dt()) } else { None }).min()
        });
        first_by_node.iter().min()
    }

    fn get_pin_collection(&self, name: &str, pin: u8) -> Option<&PinCollection>
    {
        self.states.get(name).and_then(|nodes| nodes.get(pin))
    }
}
