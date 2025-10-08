# LoRa Enabled Wireless Off-Grid Communication (LEWOC)

Haiiiii!!! This is the repo for ALL the code for LEWOC.

## Required Hardware

- [RP Pico 2W](https://www.adafruit.com/product/6315)
- [RFM95W LoRa Radio](https://www.adafruit.com/product/3072)

## Assembly

1. Attach an antenna to the radio [following this guide from Adafruit](https://learn.adafruit.com/adafruit-rfm69hcw-and-rfm96-rfm95-rfm98-lora-packet-padio-breakouts/assembly)
2. Wire together the Pico 2W and the radio (exact pins to use coming soon)

## Required Software

1. Install Rust
2. Add the `thumbv8m.main-none-eabihf` compilation target by running `rustup target add thumbv8m.main-none-eabihf`
3. Install `picotool` ([from here](https://github.com/raspberrypi/picotool) or `brew install picotool` on mac or use whatever idc)

## Flashing & Running

1. Clone this repo
2. Run `cargo run` in the root directory. This will compile the program and flash it to a connected pico in _BOOTSEL mode_. You can enter this mode by holding the BOOTSEL button when you plug in the pico or reset it.
3. Now the board should turn on the LED or something to let you know its on! If not, you can debug it by using a serial monitor (like my own creation [picocom](https://github.com/tsar-boomba/picocom)) to check the logs it sends over USB.
