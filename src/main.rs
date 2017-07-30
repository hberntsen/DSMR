extern crate time;
#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate hyper;
extern crate futures;
extern crate tokio_core;
use hyper::Client;
use hyper::mime::{Mime};
use tokio_core::reactor::Core;
use std::net::UdpSocket;
use std::mem;
use hyper::{Method, Request};
use hyper::header::{ContentLength, ContentType};
header! { (XAuthKey, "X-AUTHKEY") => [String] }
use std::str;
use std::env;

#[derive(Debug)]
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

fn to_formatted_time(year: u8, rest: u32) -> Result<std::string::String, time::ParseError> {
    let tm = to_tm(year, rest)?;
    time::strftime("%Y-%m-%dT%H:%M:%S", &tm)
}

fn format_u32(input: u32) -> std::string::String {
    format!("{}.{}", input / 1000, input % 1000)
}

fn serve() -> Result<(),std::io::Error> {
    let socket = UdpSocket::bind("0.0.0.0:37678")?;
    let mut core = Core::new().expect("Could not create core");
    let client = Client::new(&core.handle());
    let authkey = env::var("AUTH_KEY").expect("Please set the AUTH_KEY environment variable");

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
                if let Err(_) = energy_timestamp {
                        continue;
                }
                let energy_timestamp = energy_timestamp.unwrap();

                let data = format!( "power currently_delivered={},delivered_1={},delivered_2={},currently_phase_l1={},currently_phase_l2={},currently_phase_l3={} {}", ud.power_delivered, ud.energy_delivered_tariff1, ud.energy_delivered_tariff2, ud.power_delivered_l1, ud.power_delivered_l2, ud.power_delivered_l3, energy_timestamp.to_utc().to_timespec().sec);
                let uri = "http://influxdb:8086/write?db=dsmr&precision=s".parse().unwrap();
                let mime: Mime = "application/octet-stream".parse().unwrap();
                let mut req = Request::new(Method::Post, uri);
                req.headers_mut().set(ContentType(mime));
                req.headers_mut().set(ContentLength(data.len() as u64));
                req.set_body(data);

                let post = client.request(req);
                let res = core.run(post).expect("core run");
                if res.status() != hyper::StatusCode::NoContent {
                    println!("POST power: {}", res.status());
                }

            }
            {
                let gas_timestamp = to_tm(ud.gas_timestamp_year, ud.gas_timestamp_rest);
                if let Err(_) = gas_timestamp {
                        continue;
                }
                let gas_timestamp = gas_timestamp.unwrap();
            
                let data = format!( "gas delivered={} {}", ud.gas_delivered, gas_timestamp.to_utc().to_timespec().sec);
                let uri = "http://influxdb:8086/write?db=dsmr&precision=s".parse().unwrap();
                let mime: Mime = "application/octet-stream".parse().unwrap();
                let mut req = Request::new(Method::Post, uri);
                req.headers_mut().set(ContentType(mime));
                req.headers_mut().set(ContentLength(data.len() as u64));
                req.set_body(data);

                let post = client.request(req);
                let res = core.run(post).expect("core run");
                if res.status() != hyper::StatusCode::NoContent {
                    println!("POST gas: {}", res.status());
                }
            }
       }

       // dsmr-reader
       let formatted_timestamp = to_formatted_time(ud.timestamp_year, ud.timestamp_rest);
       if let Err(_) = formatted_timestamp {
            continue;
       }
       let formatted_gas_timestamp = to_formatted_time(ud.gas_timestamp_year, ud.gas_timestamp_rest);
       if let Err(_) = formatted_gas_timestamp {
            continue;
       }

       let formatted_timestamp = formatted_timestamp.unwrap();
       let formatted_gas_timestamp = formatted_gas_timestamp.unwrap();
       let json = json!({
            "timestamp": formatted_timestamp,
            "electricity_currently_delivered": format_u32(ud.power_delivered),
            "electricity_currently_returned": format_u32(ud.power_returned),
            "electricity_delivered_1": format_u32(ud.energy_delivered_tariff1),
            "electricity_delivered_2": format_u32(ud.energy_delivered_tariff2),
            "electricity_returned_1": format_u32(ud.energy_returned_tariff1),
            "electricity_returned_2": format_u32(ud.energy_returned_tariff2),
            "phase_currently_delivered_l1": format_u32(ud.power_delivered_l1),
            "phase_currently_delivered_l2": format_u32(ud.power_delivered_l2),
            "phase_currently_delivered_l3": format_u32(ud.power_delivered_l3),
            "extra_device_timestamp": formatted_gas_timestamp,
            "extra_device_delivered": format_u32(ud.gas_delivered),
       }).to_string();

        let uri = "http://dsmr-reader/api/v2/datalogger/dsmrreading".parse().unwrap();
        let mut req = Request::new(Method::Post, uri);
        req.headers_mut().set(ContentType::json());
        req.headers_mut().set(ContentLength(json.len() as u64));
        req.headers_mut().set(XAuthKey(authkey.clone()));
        req.set_body(json);

        let post = client.request(req);
        let res = core.run(post).expect("core run");
        if res.status() != hyper::StatusCode::Created {
            println!("POST dsmr-reader: {}", res.status());
        }
    }
}

//fn post(json: std::string::String) -> Result<(), hyper::error::UriError> {


    //println!("POST");

    //Ok(())
//}
