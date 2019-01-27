extern crate time;
extern crate tokio;
extern crate hyper;
mod meter;

#[macro_use] extern crate futures;
use hyper::http::header;
use tokio::net::UdpSocket;
use std::mem;
use hyper::Request;
use futures::prelude::*;
use std::io;
use std::net::{SocketAddr, Ipv4Addr};
use meter::{UsageData, USAGEDATA_SIZE};

type HC = hyper::Client<hyper::client::HttpConnector, hyper::Body>;

fn main() -> Result<(), io::Error> {
    let addr = std::net::IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
    let socket = UdpSocket::bind(&SocketAddr::new(addr, 37678))?;
    let server = Server {
        socket,
    };
    tokio::run(server.map_err(|e| eprintln!("Serve error: {:?}", e)));
    Ok(())
}


fn influx_post(data: String) -> Request<hyper::Body> {
    Request::post("http://influxdb:8086/write?db=dsmr&precision=s")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, data.len())
        .body(data.into())
        .unwrap()
}

fn influx_energy_data(ud: &UsageData, energy_timestamp: &time::Tm) -> String {
    format!( "power currently_delivered={},delivered_1={},delivered_2={},currently_phase_l1={},currently_phase_l2={},currently_phase_l3={} {}", ud.power_delivered, ud.energy_delivered_tariff1, ud.energy_delivered_tariff2, ud.power_delivered_l1, ud.power_delivered_l2, ud.power_delivered_l3, energy_timestamp.to_utc().to_timespec().sec)
}

fn influx_gas_data(ud: &UsageData, gas_timestamp: &time::Tm) -> String {
    format!( "gas delivered={} {}", ud.gas_delivered, gas_timestamp.to_utc().to_timespec().sec)
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
}

impl Future for Server {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        loop {
            let mut buf = [0; USAGEDATA_SIZE];
            let bytes_read = try_ready!(self.socket.poll_recv(&mut buf));

            if bytes_read != USAGEDATA_SIZE {
                eprintln!("Incorrect size received");
                continue;
            }

            let ud: UsageData = unsafe { mem::transmute::<[u8; USAGEDATA_SIZE], UsageData>(buf)};
            let energy_timestamp = match ud.energy_timestamp() {
                Err(e) => {
                    eprintln!("Could not read energy timestamp: {:?}", e);
                    continue;
                },
                Ok(x) => x
            };
            let gas_timestamp = match ud.gas_timestamp() {
                Err(e) => {
                    eprintln!("Could not read gas timestamp: {:?}", e);
                    continue;
                },
                Ok(x) => x
            };

            let ied = influx_energy_data(&ud, &energy_timestamp);
            let igd = influx_gas_data(&ud, &gas_timestamp);

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
