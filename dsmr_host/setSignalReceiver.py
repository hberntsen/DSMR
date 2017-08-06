#/usr/bin/env python3
import socket

ESP_IP = "192.168.123.195"
ESP_PORT = 8000

HOST_PORT = 37678
MESSAGE = bytes([192, 168, 50, 204, HOST_PORT & 0xff, HOST_PORT >> 8])

s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.connect((ESP_IP, ESP_PORT))
s.send(MESSAGE)
s.close()
