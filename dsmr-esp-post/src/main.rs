extern crate time;
extern crate tokio;
extern crate hyper;
#[macro_use] extern crate futures;
use hyper::http::header;
use tokio::net::UdpSocket;
use std::mem;
use hyper::Request;
use futures::prelude::*;
use std::io;
use std::net::{SocketAddr, Ipv4Addr};

#[repr(C,packed)]
struct UsageData {
  timestamp_year: u8,
  timestamp_rest: u32,
  power_delivered: u32,
  power_returned: u32,
  energy_delivered_tariff1: u32,
  energy_delivered_tariff2: u32,
  energy_returned_tariff1: u32,
  energy_returned_tariff2: u32,
  power_delivered_l1: u32,
  power_delivered_l2: u32,
  power_delivered_l3: u32,
  gas_timestamp_year: u8,
  gas_timestamp_rest: u32,
  gas_delivered: u32,
}

const USAGEDATA_SIZE: usize = 50;

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

fn to_tm(year: u8, rest: u32) -> Result<time::Tm, time::ParseError> {
    let mut rest_str = rest.to_string();
    if rest_str.len() < 10 {
        rest_str = format!("0{}", rest_str);
    }
    let time_str = format!("20{}{}", year.to_string(), rest_str);
    let mut time = time::strptime(&time_str, "%Y%m%d%H%M%S")?;
    // this may generate a few datapoints in the wrong timezone but we can live
    // with that
    time.tm_utcoff = time::now().tm_utcoff;
    time.tm_isdst = time::now().tm_isdst;
    Ok(time)
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
            let energy_timestamp = match to_tm(ud.timestamp_year, ud.timestamp_rest) {
                Err(e) => {
                    eprintln!("Could not read energy timestamp: {:?}", e);
                    continue;
                },
                Ok(x) => x
            };
            let gas_timestamp = match to_tm(ud.gas_timestamp_year, ud.gas_timestamp_rest) {
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
