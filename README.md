# Control heating system with mqtt messages

* arduino/esp clients sends messages with their pin statuses/readings
* system reacts according to configuration by sending messages to the control nodes

## Dependencies

* mqtt clients providing messages
* mqtt clients reacting to messages

## How it works

* arduino sends pin statuses to this application e.g. "heating/nodes/bedroom/current/temperature/3" 20.52, "heating/nodes/main-control/current/analog/32" 300 
* this application reacts/sends mqtt messages using configuration e.g "heating/nodes/master/set/json" {"pin": 3, "set": 1}
* arduino reacts by turning those pins on/off

## Howto run

```
cargo test
cargo build --release

# provide your own configuration. example src/config.yml

./target/release/heating-control --config src/config.yml

```

## Make it permanent

### systemctl

```
# become root

sudo bash

# change according to your needs

USER="tomas" CONFIG_PATH="`pwd`/src/config.yml" BIN_PATH="`pwd`/target/release/heading-control`" envsubst < "services/heating-control.service" > /etc/systemd/system/heating-control.service

systemctl daemon-reload

systemctl enable heating-control
```
