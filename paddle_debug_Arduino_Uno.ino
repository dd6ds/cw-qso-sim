// Paddle Connection Debug Test - Arduino Uno version
// This sends MIDI Note messages when paddles are pressed
// LEFT paddle -> Note 60 (Middle C)
// RIGHT paddle -> Note 62 (D)
// LED on pin 13 lights when pressed

#include <MIDI.h>

#define paddleLeft   2     // LEFT paddle
#define paddleRight  3     // RIGHT paddle
#define LED          13    // Onboard LED

bool lastLeftState = false;
bool lastRightState = false;

// Create a MIDI interface instance
MIDI_CREATE_DEFAULT_INSTANCE();

void setup() {
  pinMode(paddleLeft, INPUT_PULLUP);
  pinMode(paddleRight, INPUT_PULLUP);
  pinMode(LED, OUTPUT);

  MIDI.begin(MIDI_CHANNEL_OMNI);

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
  // Read paddles (LOW = pressed)
  bool leftPressed = !digitalRead(paddleLeft);
  bool rightPressed = !digitalRead(paddleRight);

  // LEFT paddle changed
  if (leftPressed != lastLeftState) {
    if (leftPressed) {
      MIDI.sendNoteOn(60, 100, 1);   // Note ON
      digitalWrite(LED, HIGH);
    } else {
      MIDI.sendNoteOff(60, 0, 1);    // Note OFF
      digitalWrite(LED, LOW);
    }
    lastLeftState = leftPressed;
    delay(10);                       // Simple debounce
  }

  // RIGHT paddle changed
  if (rightPressed != lastRightState) {
    if (rightPressed) {
      MIDI.sendNoteOn(62, 100, 1);
      digitalWrite(LED, HIGH);
    } else {
      MIDI.sendNoteOff(62, 0, 1);
      digitalWrite(LED, LOW);
    }
    lastRightState = rightPressed;
    delay(10);
  }

  delay(1);
}

