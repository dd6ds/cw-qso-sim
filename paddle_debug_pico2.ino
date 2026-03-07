// Paddle Connection Debug Test - Raspberry Pi Pico 2 (RP2350)
// LEFT paddle  (Dit) on GP2 -> sends Note 1
// RIGHT paddle (Dah) on GP0 -> sends Note 2
// Onboard LED: GP25 (LED_BUILTIN)
//
// Arduino IDE setup:
//   Board:     Raspberry Pi Pico 2  (Earle Philhower board package)
//   USB Stack: Adafruit TinyUSB     <<<  Tools -> USB Stack -> Adafruit TinyUSB
//
// Required libraries (install via Library Manager):
//   - Adafruit TinyUSB Library  (Adafruit)
//   - MIDI Library              (FortySevenEffects)

#include <Adafruit_TinyUSB.h>
#include <MIDI.h>

Adafruit_USBD_MIDI usb_midi;
MIDI_CREATE_INSTANCE(Adafruit_USBD_MIDI, usb_midi, MIDI);

#define paddleLeft   2           // GP2
#define paddleRight  0           // GP0
#define LED          LED_BUILTIN // GP25 on Pico 2

bool lastLeftState  = false;
bool lastRightState = false;

void setup() {
  usb_midi.begin();
  MIDI.begin(MIDI_CHANNEL_OMNI);

  pinMode(paddleLeft,  INPUT_PULLUP);
  pinMode(paddleRight, INPUT_PULLUP);
  pinMode(LED, OUTPUT);

  // Wait for USB enumeration
  while (!TinyUSBDevice.mounted()) delay(1);

  // Startup: 5 fast blinks
  for (int i = 0; i < 5; i++) {
    digitalWrite(LED, HIGH);
    delay(80);
    digitalWrite(LED, LOW);
    delay(80);
  }

  delay(500);
}

void loop() {
  MIDI.read();

  // INPUT_PULLUP: LOW = pressed, HIGH = released
  bool leftPressed  = !digitalRead(paddleLeft);
  bool rightPressed = !digitalRead(paddleRight);

  // LEFT paddle (Dit)
  if (leftPressed != lastLeftState) {
    if (leftPressed) {
      MIDI.sendNoteOn(1, 100, 1);   // Note 1 ON, velocity 100, channel 1
      digitalWrite(LED, HIGH);
    } else {
      MIDI.sendNoteOn(1, 0, 1);     // Note 1 OFF (velocity 0)
      digitalWrite(LED, LOW);
    }
    lastLeftState = leftPressed;
    delay(10); // debounce
  }

  // RIGHT paddle (Dah)
  if (rightPressed != lastRightState) {
    if (rightPressed) {
      MIDI.sendNoteOn(2, 100, 1);   // Note 2 ON, velocity 100, channel 1
      digitalWrite(LED, HIGH);
    } else {
      MIDI.sendNoteOn(2, 0, 1);     // Note 2 OFF (velocity 0)
      digitalWrite(LED, LOW);
    }
    lastRightState = rightPressed;
    delay(10); // debounce
  }

  delay(1);
}
