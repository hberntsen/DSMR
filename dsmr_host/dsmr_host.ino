#include "dsmr.h"
#include <ESP8266WiFi.h> 
#include "WiFiManager.h"          //https://github.com/tzapu/WiFiManager which was modded to support upload
#include <WiFiUdp.h>
#include <LittleFS.h>

/**
 * Define the data we're interested in, as well as the datastructure to
 * hold the parsed data. This list shows all supported fields, remove
 * any fields you are not using from the below list to make the parsing
 * and printing code smaller.
 * Each template argument below results in a field of the same name.
 */
using MyData = ParsedData<
  /* String */ identification,
  /* String */ p1_version,
  /* String */ timestamp,
  /* String */ equipment_id,
  /* FixedValue */ energy_delivered_tariff1,
  /* FixedValue */ energy_delivered_tariff2,
  /* FixedValue */ energy_returned_tariff1,
  /* FixedValue */ energy_returned_tariff2,
  /* String */ electricity_tariff,
  /* FixedValue */ power_delivered,
  /* FixedValue */ power_returned,
//  /* FixedValue */ electricity_threshold,
//  /* uint8_t */ electricity_switch_position,
  /* uint32_t */ electricity_failures,
  /* uint32_t */ electricity_long_failures,
  /* String */ electricity_failure_log,
  /* uint32_t */ electricity_sags_l1,
  /* uint32_t */ electricity_sags_l2,
  /* uint32_t */ electricity_sags_l3,
  /* uint32_t */ electricity_swells_l1,
  /* uint32_t */ electricity_swells_l2,
  /* uint32_t */ electricity_swells_l3,
//  /* String */ message_short,
  /* String */ message_long,
  /* FixedValue */ voltage_l1,
  /* FixedValue */ voltage_l2,
  /* FixedValue */ voltage_l3,
  /* FixedValue */ current_l1,
  /* FixedValue */ current_l2,
  /* FixedValue */ current_l3,
  /* FixedValue */ power_delivered_l1,
  /* FixedValue */ power_delivered_l2,
  /* FixedValue */ power_delivered_l3,
  /* FixedValue */ power_returned_l1,
  /* FixedValue */ power_returned_l2,
  /* FixedValue */ power_returned_l3,
  /* uint16_t */ gas2_device_type,
  /* String */ gas2_equipment_id,
//  /* uint8_t */ gas_valve_position,
  /* TimestampedFixedValue */ gas2_delivered
//  /* uint16_t */ thermal_device_type,
//  /* String */ thermal_equipment_id,
//  /* uint8_t */ thermal_valve_position,
//  /* TimestampedFixedValue */ thermal_delivered,
//  /* uint16_t */ water_device_type,
//  /* String */ water_equipment_id,
//  /* uint8_t */ water_valve_position,
//  /* TimestampedFixedValue */ water_delivered,
//  /* uint16_t */ slave_device_type,
//  /* String */ slave_equipment_id,
//  /* uint8_t */ slave_valve_position,
//  /* TimestampedFixedValue */ slave_delivered
>;

#define PIN_TX 1
#define PIN_RX 3
#define RESET_SSID "DSMR Relay"
#define RESET_PASSWORD "goudfish"

struct __attribute__((packed)) UsageData {
  uint8_t timestamp_year;
  uint32_t timestamp_rest;
  uint32_t power_delivered;
  uint32_t power_returned;
  uint32_t energy_delivered_tariff1;
  uint32_t energy_delivered_tariff2;
  uint32_t energy_returned_tariff1;
  uint32_t energy_returned_tariff2;
  uint32_t power_delivered_l1;
  uint32_t power_delivered_l2;
  uint32_t power_delivered_l3;
  uint8_t gas_timestamp_year;
  uint32_t gas_timestamp_rest;
  uint32_t gas_delivered;
};
static_assert(sizeof(UsageData) == 50, "UsageData size mismatch");

// Set up to read from the second serial port, and use D2 as the request
// pin. On boards with only one (USB) serial port, you can also use
// SoftwareSerial.
P1Reader reader(&Serial, PIN_TX);
WiFiServer server(8000);
WiFiClient client;
unsigned long last;
WiFiUDP udp;

//settings
static IPAddress receiverIP;
static uint16_t receiverPort = 0;
static uint16_t interval = 10000;

void ledToggle() {
  digitalWrite(PIN_TX, !digitalRead(PIN_TX));
}

void setup() {
  WiFi.hostname(RESET_SSID);
  // If we go to reset, do a blocking portal
  pinMode(2, INPUT);  
  pinMode(0, OUTPUT);
  digitalWrite(0, 0);
  bool resetPressed = !digitalRead(2);
  pinMode(0, INPUT);
  
  if(resetPressed) {
    LittleFS.format();
    WiFiManager wifiManager;
    wifiManager.startConfigPortal(RESET_SSID, RESET_PASSWORD);
  }

  if(!LittleFS.begin()) {
    LittleFS.format();
    LittleFS.begin();
  }
  
  WiFi.mode(WIFI_STA);
  //Blink while we are connecting
  while (WiFi.status() != WL_CONNECTED) {
    delay(100);
    ledToggle();
  }
 
  Serial.begin(115200);
  pinMode(PIN_TX, OUTPUT);

  getReceiver(receiverIP, receiverPort);

  server.begin();
  server.setNoDelay(true);
  // start a read right away
  reader.enable(true);
  last = millis();
}


int readBlocking(WiFiClient& wifiClient, uint8_t* buf, size_t size, int16_t timeout) {
  uint16_t timeWaited = 0;
  size_t totalBytesRead = 0;
  bool first = true;

  while(totalBytesRead < size && timeWaited < timeout) {
    if(!first) {
      delay(1); 
      timeWaited++;
    }
    first = false;
    
    int bytesRead = wifiClient.read(buf + totalBytesRead, size - totalBytesRead);
    if(bytesRead > 0) {
      totalBytesRead += bytesRead;
    }
  }
  return totalBytesRead;
}

void handleSetReceiver() {
  uint8_t data[6];
  readBlocking(client, data, 6, 1000);
  auto file = LittleFS.open("/receiver", "w+");
  file.write(data, 6);
  file.close();

  receiverIP = IPAddress(*((uint32_t*)data));
  receiverPort = *((uint16_t*)(data + 4));

  client.write('k');
}

void handleSetInterval() {
  uint8_t data[2];
  readBlocking(client, data, 2, 1000);
  auto file = LittleFS.open("/interval", "w+");
  file.write(data, 2);
  file.close();

  interval = *((uint16_t*)data);

  client.write('k');
}


void handleConnection() {
  uint8_t type;
  readBlocking(client, &type, 1, 1000);
  switch(type) {
    case 'r':
    handleSetReceiver();
    break;
    case 'i':
    handleSetInterval();
    break;
    default:
    client.write('e');
  }
}

bool getReceiver(IPAddress& ip, uint16_t& port) {
  File file = LittleFS.open("/receiver", "r");
  if(!file) {
    return false;
  }
  char bytes[6];
  file.readBytes(bytes, 6);
  ip = IPAddress(*((uint32_t*)bytes));
  port = *((uint16_t*)(bytes + 4));
  file.close();
  return true;
}

bool getInterval() {
  File file = LittleFS.open("/interval", "r");
  if(!file) {
    return false;
  }
  file.readBytes((char*)&interval, 2);
  file.close();
  return true;
}

void convert_timestamp(String& timestamp, uint8_t* out_year, uint32_t* out_rest) {
  char yearstr[3];
  yearstr[2] = 0;
  memcpy(yearstr, timestamp.c_str(), 2);
  *out_year = atoi(yearstr);
  
  char reststr[11];
  reststr[10] = 0;
  memcpy(reststr, timestamp.c_str()+2, 10);
  uint32_t restint = strtoul(reststr, NULL, 10);
  memcpy(out_rest, &restint, 4);
}

void loop () {
  // Blink LED when connecting
  if( WiFi.status() != WL_CONNECTED) {
    ledToggle();
    delay(75);
  } else {
    //check if there are any new clients
    if (server.hasClient()){
        //free/disclient client
        if (!client || !client.connected()){
          if(client) {
            client.stop();
          }
          client = server.available();
      }
      //no free/disclient spot so reject
      WiFiClient serverClient = server.available();
      serverClient.stop();
    }
  
    //check wifi client for data
    if (client && client.connected() && client.available()){
      handleConnection();
      client.stop();
    }

    // Allow the reader to check the serial buffer regularly
    reader.loop();
    
    // Every 10 sec, fire off a one-off reading
    unsigned long now = millis();
    if (now - last > interval) {
      reader.enable(true);
      last = now;
    }
  
    if (reader.available()) {
      MyData data;
      String err;
      if (reader.parse(&data, &err)) {
        UsageData ud;
        convert_timestamp(data.timestamp, &ud.timestamp_year, &ud.timestamp_rest);
        ud.power_delivered = data.power_delivered.int_val();
        ud.power_returned = data.power_returned.int_val();
        ud.energy_delivered_tariff1 = data.energy_delivered_tariff1.int_val();
        ud.energy_delivered_tariff2 = data.energy_delivered_tariff2.int_val();
        ud.energy_returned_tariff1 = data.energy_returned_tariff1.int_val();
        ud.energy_returned_tariff2 = data.energy_returned_tariff2.int_val();
        ud.power_delivered_l1 = data.power_delivered_l1.int_val();
        ud.power_delivered_l2 = data.power_delivered_l2.int_val();
        ud.power_delivered_l3 = data.power_delivered_l3.int_val();
        convert_timestamp(data.gas2_delivered.timestamp, &ud.gas_timestamp_year, &ud.gas_timestamp_rest);
        ud.gas_delivered = data.gas2_delivered.int_val();

        if(receiverPort) {
            udp.beginPacket(receiverIP, receiverPort);
            udp.write((uint8_t*) &ud, sizeof(UsageData));
            udp.endPacket();
        }
      } else {
        // Parser error, print error
        //Serial.println(err);
      }
    }
  }
}
