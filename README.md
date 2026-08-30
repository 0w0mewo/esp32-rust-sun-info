## ESP32 rust - sun info

- Sync current datetime with NTP
- Display current datetime, julian days, time of sunrise, time of sunset, sun position on SSD1306

#### Hardware requirements
- any ESP32 dev board with 4MB flash
- SSD1306 with I2C interface
- button
- WS2812B LED

#### Connectivity
| ESP32 IO pin | Target module pin |
|----|----|
| GPIO18 | SSD1306 SCL |
| GPIO23 | SSD1306 SDA |
| GPIO19 | Button, connect another pin to GND |
| GPIO33 | WS2812 DIN |