# Control heating system with mqtt messages

* arduino/esp clients sends messages with their pin statuses/readings
* system reacts according to configuration by sending messages to control nodes

## Dependencies

* mqtt clients providing messages
* mqtt clients reacting to messages

## How it works

* based on config.yml system reacts/sends mqtt messages to the main controller e.g "heating/nodes/master/set/json" {"pin": 3, "set": 1}
* controller reacts by turning those pins on/off
* controller sends messages with the pin/sensor values "heating/nodes/bedroom/current/temperature/3" "20.5"
* system controls

## Howto run

```
cargo test
cargo build --release

# provide your own configuration. example src/config.yml

./target/release/heating-control --config src/config.yml

```

## Make it pernament

### systemctl

```
# become root

sudo bash

# change according to your needs

USER="tomas" CONFIG_PATH="`pwd`/src/config.yml" BIN_PATH="`pwd`/target/release/heading-control`" envsubst < "services/heating-control.service" > /etc/systemd/system/heating-control.service

systemctl daemon-reload

systemctl enable heating-control
```
