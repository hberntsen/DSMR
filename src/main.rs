extern crate time;
#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate hyper;
extern crate futures;
extern crate tokio_core;
use futures::{Future, Stream};
use hyper::Client;
use tokio_core::reactor::Core;
use std::net::UdpSocket;
use std::mem;
use hyper::{Method, Request};
use hyper::header::{ContentLength, ContentType, Headers};
header! { (XAuthKey, "X-AUTHKEY") => [String] }



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
    serve().expect("Error");
}

fn to_tm(year: u8, rest: u32) -> Result<time::Tm, time::ParseError> {
    let mut rest_str = rest.to_string();
    if rest_str.len() < 10 {
        rest_str = format!("0{}", rest_str);
    }
    let time_str = format!("20{}{}", year.to_string(), rest_str);
    time::strptime(&time_str, "%Y%m%d%H%M%S")
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

    loop {
        // read from the socket
        let mut buf = [0; USAGEDATA_SIZE];
        let bytes_read = socket.recv(&mut buf)?;
        if bytes_read != USAGEDATA_SIZE {
            println!("Incorrect size received");
            continue;
        }

       let ud: UsageData = unsafe { mem::transmute::<[u8; USAGEDATA_SIZE], UsageData>(buf)};
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
       println!("{}", json);

        let uri = "http://dsmr.aberntsen.nl/api/v2/datalogger/dsmrreading".parse().unwrap();
        let authkey = String::from("33FS0PT0CL5RY9WRMS0T8TOS01ZUQYO2RXVIRFZSURC4CQQO51GPFACZWB41CD0C");
        let mut req = Request::new(Method::Post, uri);
        req.headers_mut().set(ContentType::json());
        req.headers_mut().set(ContentLength(json.len() as u64));
        req.headers_mut().set(XAuthKey(authkey));
        req.set_body(json);

        let post = client.request(req).and_then(|res| {
            println!("POST: {}", res.status());

            res.body().concat2()
        });
        core.run(post).expect("core run");

    }
}

//fn post(json: std::string::String) -> Result<(), hyper::error::UriError> {


    //println!("POST");

    //Ok(())
//}
