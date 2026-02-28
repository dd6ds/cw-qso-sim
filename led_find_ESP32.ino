// ── ESP32 LED Finder ──────────────────────────────────────────────────────────
// Cycles through common LED GPIO pins one at a time.
// Each pin blinks its own number of times so you can identify it:
//   GPIO  2 → blinks 2 times, pause
//   GPIO  4 → blinks 4 times, pause
//   GPIO  5 → blinks 5 times, pause
//   GPIO 16 → blinks 16 times (fast), pause
//   GPIO 22 → blinks 22 times (fast), pause
//   GPIO 23 → blinks 23 times (fast), pause
//
// Watch which GPIO makes your LED blink — that is your LED pin.
// Serial Monitor (115200 baud) also prints which GPIO is currently blinking.
//
// ── Arduino IDE setup ─────────────────────────────────────────────────────────
//   Board : ESP32 Dev Module
//   Port  : /dev/ttyUSB0
//   Serial Monitor baud : 115200

// Common LED pins across different ESP32 DevKit variants
const int LED_PINS[] = { 2, 4, 5, 16, 22, 23 };
const int PIN_COUNT  = sizeof(LED_PINS) / sizeof(LED_PINS[0]);

// Blink a pin N times (tries both HIGH and LOW so active-high and active-low both work)
void blinkPin(int pin, int times) {
  // Try active HIGH first
  for (int i = 0; i < times; i++) {
    digitalWrite(pin, HIGH);
    delay(times > 10 ? 80 : 150);
    digitalWrite(pin, LOW);
    delay(times > 10 ? 80 : 150);
  }
  delay(300);
  // Try active LOW (for boards where LED cathode is on GPIO)
  for (int i = 0; i < times; i++) {
    digitalWrite(pin, LOW);
    delay(times > 10 ? 80 : 150);
    digitalWrite(pin, HIGH);
    delay(times > 10 ? 80 : 150);
  }
}

void setup() {
  Serial.begin(115200);
  delay(500);

  // Set all candidate pins as OUTPUT, start LOW
  for (int i = 0; i < PIN_COUNT; i++) {
    pinMode(LED_PINS[i], OUTPUT);
    digitalWrite(LED_PINS[i], LOW);
  }

  Serial.println("=== ESP32 LED Finder ===");
  Serial.println("Watch which GPIO makes your LED blink.");
  Serial.println();
}

void loop() {
  for (int i = 0; i < PIN_COUNT; i++) {
    int pin = LED_PINS[i];

    Serial.print("Testing GPIO ");
    Serial.print(pin);
    Serial.print(" — blinks ");
    Serial.print(pin);
    Serial.println(" times ...");

    blinkPin(pin, pin > 10 ? 6 : pin);  // cap long pins to 6 blinks for readability

    // Rest between pins — all LOW
    for (int j = 0; j < PIN_COUNT; j++) {
      digitalWrite(LED_PINS[j], LOW);
    }

    Serial.print("GPIO ");
    Serial.print(pin);
    Serial.println(" done. Pause 1.5s ...");
    delay(1500);
  }

  Serial.println("--- Full cycle done, repeating ---");
  Serial.println();
}
