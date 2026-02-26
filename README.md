A simple TUI CW QSO Simulator/Tainer


Supported Adapter: 
- ATTiny85 / Digispark
- vBand USB Adapter


Debian Linux / Linux Mint

/etc/udev/rules.d/49-digispark.rules

# Digispark ATtiny85
SUBSYSTEMS=="usb", ATTRS{idVendor}=="16d0", ATTRS{idProduct}=="0753", MODE:="0666"
KERNEL=="ttyACM*", ATTRS{idVendor}=="16d0", ATTRS{idProduct}=="0753", MODE:="0666", ENV{ID_MM_DEVICE_IGNORE}="1"

# Digispark ATtiny167
SUBSYSTEMS=="usb", ATTRS{idVendor}=="16d0", ATTRS{idProduct}=="0753", MODE:="0666"


Wiring:

ATtiny85 Pin 2 (P2)  →  LEFT paddle (Dit)
ATtiny85 Pin 0 (P0)  →  RIGHT paddle (Dah)
ATtiny85 GND         →  Paddle common ground


Programming the ATtiny85:

    Install Arduino IDE (available for all platforms)
    Add Digispark board support:
        File → Preferences → Additional Board Manager URLs
        Add: [http://digistump.com/package_digistump_index.json](https://raw.githubusercontent.com/digistump/arduino-boards-index/master/package_digistump_index.json)
    Tools → Board → Board Manager → Install "Digistump AVR Boards"
    Install DigiMIDI library (Sketch → Include Library → Manage Libraries) https://github.com/heartscrytech/DigisparkMIDI
    Open paddle_decoder.ino
    Tools → Board → Digispark (Default - 16.5mhz)
    Sketch → Upload (plug in Digispark when prompted)





! VIPE Coded !
