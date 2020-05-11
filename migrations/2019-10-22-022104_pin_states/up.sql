CREATE TABLE pin_states (
  id VARCHAR(255) NOT NULL PRIMARY KEY,
  name VARCHAR(255) NOT NULL,
  pin INTEGER(1) NOT NULL,
  input_type INTEGER(1) NOT NULL DEFAULT 0,
  value INTEGER(2) NOT NULL DEFAULT 0,
  dtc VARCHAR(255) NOT NULL
);
CREATE INDEX pin_states_name_pin_index ON pin_states (name, pin);
CREATE INDEX pin_states_dtm_index ON pin_states (dtc);
