// Paddle Connection Debug Test
// This sends MIDI messages to help diagnose wiring
// P2 = LEFT paddle (Dit) sends Note 1
// P0 = RIGHT paddle (Dah) sends Note 2

#include <DigiMIDI.h>

#define paddleLeft   2    // P2
#define paddleRight  0    // P0
#define LED          1    // P1

bool lastLeftState = false;
bool lastRightState = false;

void setup() {
  pinMode(paddleLeft, INPUT_PULLUP);
  pinMode(paddleRight, INPUT_PULLUP); 
  pinMode(LED, OUTPUT);
  
  // Startup: 5 fast blinks
  for (int i = 0; i < 5; i++) {
    digitalWrite(LED, HIGH);
    DigiMIDI.delay(80);
    digitalWrite(LED, LOW);
    DigiMIDI.delay(80);
  }
  
  DigiMIDI.delay(500);
}

void loop() {
  DigiMIDI.update();
  
  // Read paddles (INPUT_PULLUP means: LOW = pressed, HIGH = not pressed)
  bool leftPressed = !digitalRead(paddleLeft);   // Invert because of pullup
  bool rightPressed = !digitalRead(paddleRight); // Invert because of pullup
  
  // LEFT paddle changed
  if (leftPressed != lastLeftState) {
    if (leftPressed) {
      // LEFT pressed - send Note 1 ON (velocity 100)
      DigiMIDI.sendNoteOn(1, 100, 1);
      digitalWrite(LED, HIGH);
    } else {
      // LEFT released - send Note 1 OFF (velocity 0)
      DigiMIDI.sendNoteOn(1, 0, 1);
      digitalWrite(LED, LOW);
    }
    lastLeftState = leftPressed;
    DigiMIDI.delay(10);  // Small debounce
  }
  
  // RIGHT paddle changed
  if (rightPressed != lastRightState) {
    if (rightPressed) {
      // RIGHT pressed - send Note 2 ON (velocity 100)
      DigiMIDI.sendNoteOn(2, 100, 1);
      digitalWrite(LED, HIGH);
    } else {
      // RIGHT released - send Note 2 OFF (velocity 0)
      DigiMIDI.sendNoteOn(2, 0, 1);
      digitalWrite(LED, LOW);
    }
    lastRightState = rightPressed;
    DigiMIDI.delay(10);  // Small debounce
  }
  
  DigiMIDI.delay(1);
}

