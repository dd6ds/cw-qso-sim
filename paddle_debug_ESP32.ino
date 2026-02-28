// Paddle Connection Debug Test — ESP32 version
// Sends raw MIDI Note bytes over UART0 (USB-to-serial) at 115200 baud.
// No external library needed — uses Serial.write() directly.
// Note: uses 115200 instead of standard MIDI 31250 baud because 31250 is
// unreliable on Linux with CP2102/CH340 USB-serial chips.
//
// cw-qso-sim reads these with:
//   --adapter esp32 --port /dev/ttyUSB0
//
// MIDI messages sent:
//   DIT press   : 0x90 0x3C 0x64  (NoteOn  ch1 note60 vel100)
//   DIT release : 0x80 0x3C 0x00  (NoteOff ch1 note60 vel0)
//   DAH press   : 0x90 0x3E 0x64  (NoteOn  ch1 note62 vel100)
//   DAH release : 0x80 0x3E 0x00  (NoteOff ch1 note62 vel0)
//
// ── Wiring (ESP32-DevKitC / WROOM-32) ────────────────────────────────────────
//
//   Paddle tip (DIT)  ──── GPIO 32 ──┐
//   Paddle tip (DAH)  ──── GPIO 33   │  other end of each tip → GND
//   Paddle ring       ──── GND       │
//
//   Built-in LED      ──── GPIO 2  (active HIGH on most DevKit boards)
//
// ── Pin notes ─────────────────────────────────────────────────────────────────
//
//   GPIO  6–11  Internal flash bus — NEVER use for I/O
//   GPIO  0     Strapping pin (boot mode) — avoid for paddle
//   GPIO  12    Strapping pin (flash voltage) — avoid for paddle
//   GPIO 32     Safe on all ESP32 variants, has internal pull-up
//   GPIO 33     Safe on all ESP32 variants, has internal pull-up
//
// ── Arduino IDE setup ─────────────────────────────────────────────────────────
//
//   Board        : "ESP32 Dev Module"  (Espressif Systems esp32 board package)
//   No library needed — raw Serial.write() only
//   Upload speed : 921600 (default)
//   After upload : close Serial Monitor before running cw-qso-sim

#define paddleLeft   23    // DIT — left  paddle tip → GND
#define paddleRight  22    // DAH — right paddle tip → GND
#define LED           2    // Onboard LED (active HIGH)

#define NOTE_DIT     60    // Middle C
#define NOTE_DAH     62    // D
#define VELOCITY    100

bool lastLeftState  = false;
bool lastRightState = false;

// Send a 3-byte MIDI NoteOn message
void midiNoteOn(byte note, byte velocity) {
  Serial.write(0x90);      // NoteOn, channel 1
  Serial.write(note);
  Serial.write(velocity);
}

// Send a 3-byte MIDI NoteOff message
void midiNoteOff(byte note) {
  Serial.write(0x80);      // NoteOff, channel 1
  Serial.write(note);
  Serial.write((byte)0);
}

void setup() {
  pinMode(paddleLeft,  INPUT_PULLUP);
  pinMode(paddleRight, INPUT_PULLUP);
  pinMode(LED, OUTPUT);

  // 115200 baud — reliable on Linux with CP2102/CH340.
  // (31250 MIDI baud is unreliable on Linux with these USB chips.)
  Serial.begin(115200);

  // 5 fast blinks → confirms sketch is running on the ESP32
  for (int i = 0; i < 5; i++) {
    digitalWrite(LED, HIGH);
    delay(80);
    digitalWrite(LED, LOW);
    delay(80);
  }

  delay(200);
}

void loop() {
  // INPUT_PULLUP: idle = HIGH, pressed (shorted to GND) = LOW
  bool leftPressed  = (digitalRead(paddleLeft)  == LOW);
  bool rightPressed = (digitalRead(paddleRight) == LOW);

  // LEFT (DIT) paddle state changed
  if (leftPressed != lastLeftState) {
    if (leftPressed) {
      midiNoteOn(NOTE_DIT, VELOCITY);
      digitalWrite(LED, HIGH);
    } else {
      midiNoteOff(NOTE_DIT);
      digitalWrite(LED, LOW);
    }
    lastLeftState = leftPressed;
    delay(10);    // debounce
  }

  // RIGHT (DAH) paddle state changed
  if (rightPressed != lastRightState) {
    if (rightPressed) {
      midiNoteOn(NOTE_DAH, VELOCITY);
      digitalWrite(LED, HIGH);
    } else {
      midiNoteOff(NOTE_DAH);
      digitalWrite(LED, LOW);
    }
    lastRightState = rightPressed;
    delay(10);    // debounce
  }

  delay(1);
}
