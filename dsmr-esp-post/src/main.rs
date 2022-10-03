extern crate time;
extern crate tokio;
extern crate hyper;
extern crate paho_mqtt;
mod meter;
mod mqtt;

#[macro_use] extern crate futures;
use hyper::http::header;
use tokio::net::UdpSocket;
use hyper::Request;
use futures::prelude::*;
use std::io;
use std::net::{SocketAddr, Ipv4Addr};
use meter::{UsageData, USAGEDATARAW_SIZE};

type HC = hyper::Client<hyper::client::HttpConnector, hyper::Body>;

fn main() -> Result<(), io::Error> {
    let addr = std::net::IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
    let socket = UdpSocket::bind(&SocketAddr::new(addr, 37678))?;

    let (to_mqtt, mqtt_receiver) = futures::sync::mpsc::channel(10);
    let server = Server {
        socket,
        to_mqtt
    };

    mqtt::run(mqtt_receiver);
    tokio::run(
        server.map_err(|e| eprintln!("Serve error: {:?}", e))
    );
    Ok(())
}


fn influx_post(data: String) -> Request<hyper::Body> {
    Request::post("http://influxdb:8086/write?db=dsmr&precision=s")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, data.len())
        .body(data.into())
        .unwrap()
}

fn influx_energy_data(ud: &UsageData) -> String {
    format!( "power currently_delivered={},delivered_1={},delivered_2={},currently_phase_l1={},currently_phase_l2={},currently_phase_l3={},voltage_l1={},voltage_l2={},voltage_l3={},currently_returned={},returned_1={},returned_2={},power_returned_l1={},power_returned_l2={},power_returned_l3={} {}", ud.power_delivered, ud.energy_delivered_tariff1, ud.energy_delivered_tariff2, ud.power_delivered_l1, ud.power_delivered_l2, ud.power_delivered_l3, ud.voltage_l1, ud.voltage_l2, ud.voltage_l3, ud.power_returned, ud.energy_returned_tariff1, ud.energy_returned_tariff2, ud.power_returned_l1, ud.power_returned_l2, ud.power_returned_l3, ud.power_timestamp.to_utc().to_timespec().sec)
}

fn influx_gas_data(ud: &UsageData) -> String {
    format!( "gas delivered={} {}", ud.gas_delivered, ud.gas_timestamp.to_utc().to_timespec().sec)
}

fn influx_post_energy(client: HC, data: String) -> impl Future<Item=HC, Error=hyper::Error> {
    client.request(influx_post(data))
        .map(|res| {
            if res.status() != hyper::StatusCode::NO_CONTENT {
                println!("POST energy: {}", res.status());
            }
            client
        })
}

fn influx_post_gas(client: HC, data: String) -> impl Future<Item=HC, Error=hyper::Error> {
    client.request(influx_post(data))
        .map(|res| {
            if res.status() != hyper::StatusCode::NO_CONTENT {
                println!("POST gas: {}", res.status());
            }
            client
        })
}

struct Server {
    socket: UdpSocket,
    to_mqtt: futures::sync::mpsc::Sender<UsageData>
}

impl Future for Server {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        loop {
            let mut buf: [u8; USAGEDATARAW_SIZE] = [0; USAGEDATARAW_SIZE];
            let bytes_read = try_ready!(self.socket.poll_recv(&mut buf));

            if bytes_read != USAGEDATARAW_SIZE {
                eprintln!("Incorrect size received");
                continue;
            }

            let ud: UsageData = match UsageData::from_raw(&buf) {
                Err(e) => {
                    eprintln!("Could not read energy usage data: {:?}", e);
                    continue;
                },
                Ok(x) => x
            };

            let ied = influx_energy_data(&ud);
            let igd = influx_gas_data(&ud);
            self.to_mqtt.try_send(ud).unwrap_or_else(|e| {
                eprintln!("Could not send data to mqtt thread: {:?}", e);
            });

            tokio::spawn({
                influx_post_energy(hyper::Client::new(), ied)
                    .and_then(|client| influx_post_gas(client, igd))
                    .map(|_| ())
                    .map_err(|e| eprintln!("Influx post error: {:?}", e))
                }
            );
        }
    }
}
