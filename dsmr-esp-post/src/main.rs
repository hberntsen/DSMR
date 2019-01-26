extern crate time;

extern crate hyper;
extern crate futures;
extern crate tokio_core;
use hyper::Client;
use hyper::http::header;
use tokio_core::reactor::Core;
use std::net::UdpSocket;
use std::mem;
use hyper::Request;

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

fn main() {
    serve().expect("Serve error");
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

fn serve() -> Result<(),std::io::Error> {
    let socket = UdpSocket::bind("0.0.0.0:37678")?;
    let mut core = Core::new().expect("Could not create core");
    let client = Client::new();

    loop {
        // read from the socket
        let mut buf = [0; USAGEDATA_SIZE];
        let bytes_read = socket.recv(&mut buf)?;
        if bytes_read != USAGEDATA_SIZE {
            println!("Incorrect size received");
            continue;
        }

       let ud: UsageData = unsafe { mem::transmute::<[u8; USAGEDATA_SIZE], UsageData>(buf)};
       // influxdb
       {
            {
                let energy_timestamp = to_tm(ud.timestamp_year, ud.timestamp_rest);
                if energy_timestamp.is_err() {
                    continue;
                }
                let energy_timestamp = energy_timestamp.unwrap();

                let data = format!( "power currently_delivered={},delivered_1={},delivered_2={},currently_phase_l1={},currently_phase_l2={},currently_phase_l3={} {}", ud.power_delivered, ud.energy_delivered_tariff1, ud.energy_delivered_tariff2, ud.power_delivered_l1, ud.power_delivered_l2, ud.power_delivered_l3, energy_timestamp.to_utc().to_timespec().sec);
                let mut req = Request::post("http://influxdb:8086/write?db=dsmr&precision=s")
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .header(header::CONTENT_LENGTH, data.len())
                    .body(data.into())
                    .unwrap();

                let post = client.request(req);
                let res = core.run(post).expect("core post power");
                if res.status() != hyper::StatusCode::NO_CONTENT {
                    println!("POST power: {}", res.status());
                }
            }
            {
                let gas_timestamp = to_tm(ud.gas_timestamp_year, ud.gas_timestamp_rest);
                if gas_timestamp.is_err() {
                    continue;
                }
                let gas_timestamp = gas_timestamp.unwrap();

                let data = format!( "gas delivered={} {}", ud.gas_delivered, gas_timestamp.to_utc().to_timespec().sec);

                let mut req = Request::post("http://influxdb:8086/write?db=dsmr&precision=s")
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .header(header::CONTENT_LENGTH, data.len())
                    .body(data.into())
                    .unwrap();

                let post = client.request(req);
                let res = core.run(post).expect("core post gas");
                if res.status() != hyper::StatusCode::NO_CONTENT {
                    println!("POST gas: {}", res.status());
                }
            }
       }
    }
}
