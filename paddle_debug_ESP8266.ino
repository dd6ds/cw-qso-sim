// Paddle Connection Debug Test — ESP8266 version
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
// ── Wiring ────────────────────────────────────────────────────────────────────
//
//   Paddle tip (DIT)  ──── D5 (GPIO 14) ──┐
//   Paddle tip (DAH)  ──── D6 (GPIO 12)   │  other end of each tip → GND
//   Paddle ring       ──── GND            │
//
//   Built-in LED      ──── D4 (GPIO 2) — active LOW on most ESP8266 boards
//                          (LED ON when GPIO 2 is LOW)
//
// ── Pin notes ─────────────────────────────────────────────────────────────────
//
//   GPIO  0   (D3)  Strapping pin — avoid for paddle
//   GPIO  2   (D4)  Strapping pin / built-in LED — avoid for paddle
//   GPIO 15   (D8)  Strapping pin — avoid for paddle
//   GPIO  6–11      Internal flash bus — NEVER use for I/O
//   GPIO 14   (D5)  Safe, has internal pull-up ← DIT
//   GPIO 12   (D6)  Safe, has internal pull-up ← DAH
//   GPIO 13   (D7)  Safe alternative if D5/D6 are busy
//   GPIO  4   (D2)  Safe alternative
//   GPIO  5   (D1)  Safe alternative
//
// ── Board pinout reference ────────────────────────────────────────────────────
//
//   NodeMCU / Wemos D1 Mini label → GPIO number:
//     D0=16  D1=5   D2=4   D3=0   D4=2
//     D5=14  D6=12  D7=13  D8=15
//
// ── Arduino IDE setup ─────────────────────────────────────────────────────────
//
//   Board manager URL : https://arduino.esp8266.com/stable/package_esp8266com_index.json
//   Board (NodeMCU)   : "NodeMCU 1.0 (ESP-12E Module)"
//   Board (Wemos)     : "LOLIN(WEMOS) D1 mini"
//   No library needed — raw Serial.write() only
//   Upload speed      : 115200
//   After upload      : close Serial Monitor before running cw-qso-sim

#define paddleLeft   14    // DIT — D5 on NodeMCU/Wemos — paddle tip → GND
#define paddleRight  12    // DAH — D6 on NodeMCU/Wemos — paddle tip → GND
#define LED           2    // Built-in LED — active LOW on ESP8266

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
  digitalWrite(LED, HIGH);   // LED off (active LOW)

  // 115200 baud — reliable on Linux with CP2102/CH340
  Serial.begin(115200);

  // 5 fast blinks — confirms sketch is running
  // Note: ESP8266 built-in LED is active LOW (LOW = on, HIGH = off)
  for (int i = 0; i < 5; i++) {
    digitalWrite(LED, LOW);    // LED on
    delay(80);
    digitalWrite(LED, HIGH);   // LED off
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
      digitalWrite(LED, LOW);    // LED on (active LOW)
    } else {
      midiNoteOff(NOTE_DIT);
      digitalWrite(LED, HIGH);   // LED off
    }
    lastLeftState = leftPressed;
    delay(10);    // debounce
  }

  // RIGHT (DAH) paddle state changed
  if (rightPressed != lastRightState) {
    if (rightPressed) {
      midiNoteOn(NOTE_DAH, VELOCITY);
      digitalWrite(LED, LOW);    // LED on (active LOW)
    } else {
      midiNoteOff(NOTE_DAH);
      digitalWrite(LED, HIGH);   // LED off
    }
    lastRightState = rightPressed;
    delay(10);    // debounce
  }

  delay(1);
}
