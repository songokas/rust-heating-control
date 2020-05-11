CREATE TABLE temperatures (
  id VARCHAR(255) NOT NULL PRIMARY KEY,
  name VARCHAR(255) NOT NULL,
  pin INTEGER(1) NOT NULL,
  temperature REAL NOT NULL,
  dtc VARCHAR(255) NOT NULL
);
CREATE INDEX temp_name_pin_index ON temperatures (name, pin);
CREATE INDEX temp_dtc_index ON temperatures (dtc);