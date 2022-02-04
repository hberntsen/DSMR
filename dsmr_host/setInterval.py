#/usr/bin/env python3
import socket

ESP_IP = "192.168.46.11"
ESP_PORT = 8000

INTERVAL = 10000
MESSAGE = b'i' + bytes([INTERVAL & 0xff, INTERVAL >> 8])

s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.connect((ESP_IP, ESP_PORT))
s.send(MESSAGE)
s.close()
