// Paddle Connection Debug Test - Arduino Nano version
// ATmega328P, CH340/FT232 USB-serial chip
//
// The Nano does NOT enumerate as a native USB-MIDI device.
// MIDI bytes go out over hardware Serial (TX pin 1, 31250 baud).
// On the PC side you need one of:
//   a) Hairless MIDI <-> Serial Bridge  (hairlessmidi.sourceforge.net)
//   b) ttymidi                          (linux: ttymidi -s /dev/ttyUSB0 -b 31250)
//   c) Any other serial-to-MIDI bridge
//
// Wire:
//   DIT paddle  ->  D2  (INPUT_PULLUP, GND on press)
//   DAH paddle  ->  D3  (INPUT_PULLUP, GND on press)
//   Common GND  ->  GND
//
// MIDI messages:
//   DIT pressed  -> Note On  ch1 note 60 vel 100
//   DIT released -> Note Off ch1 note 60 vel 0
//   DAH pressed  -> Note On  ch1 note 62 vel 100
//   DAH released -> Note Off ch1 note 62 vel 0
//
// Onboard LED (D13) mirrors any paddle press.

#include <MIDI.h>

#define PIN_DIT   2    // DIT (left)  paddle
#define PIN_DAH   3    // DAH (right) paddle
#define PIN_LED   13   // Onboard LED

#define NOTE_DIT  60   // Middle C
#define NOTE_DAH  62   // D
#define MIDI_CH   1
#define MIDI_VEL  100
#define DEBOUNCE_MS 10

MIDI_CREATE_DEFAULT_INSTANCE();

bool lastDit = false;
bool lastDah = false;

// ── helpers ──────────────────────────────────────────────────────────────────

void blink(uint8_t times, uint16_t onMs, uint16_t offMs) {
  for (uint8_t i = 0; i < times; i++) {
    digitalWrite(PIN_LED, HIGH); delay(onMs);
    digitalWrite(PIN_LED, LOW);  delay(offMs);
  }
}

void updateLed(bool dit, bool dah) {
  digitalWrite(PIN_LED, (dit || dah) ? HIGH : LOW);
}

// ── setup ────────────────────────────────────────────────────────────────────

void setup() {
  pinMode(PIN_DIT, INPUT_PULLUP);
  pinMode(PIN_DAH, INPUT_PULLUP);
  pinMode(PIN_LED, OUTPUT);

  MIDI.begin(MIDI_CHANNEL_OMNI);   // starts Serial at 31250 baud

  // Startup indication: 5 fast blinks
  blink(5, 80, 80);
  delay(300);
}

// ── loop ─────────────────────────────────────────────────────────────────────

void loop() {
  bool dit = !digitalRead(PIN_DIT);   // LOW = pressed (pull-up)
  bool dah = !digitalRead(PIN_DAH);

  // ── DIT ──────────────────────────────────────────────────────────────────
  if (dit != lastDit) {
    delay(DEBOUNCE_MS);
    dit = !digitalRead(PIN_DIT);      // re-read after debounce
    if (dit != lastDit) {
      if (dit) {
        MIDI.sendNoteOn(NOTE_DIT, MIDI_VEL, MIDI_CH);
      } else {
        MIDI.sendNoteOff(NOTE_DIT, 0, MIDI_CH);
      }
      lastDit = dit;
    }
  }

  // ── DAH ──────────────────────────────────────────────────────────────────
  if (dah != lastDah) {
    delay(DEBOUNCE_MS);
    dah = !digitalRead(PIN_DAH);
    if (dah != lastDah) {
      if (dah) {
        MIDI.sendNoteOn(NOTE_DAH, MIDI_VEL, MIDI_CH);
      } else {
        MIDI.sendNoteOff(NOTE_DAH, 0, MIDI_CH);
      }
      lastDah = dah;
    }
  }

  updateLed(lastDit, lastDah);

  delay(1);
}
