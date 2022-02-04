use futures::prelude::*;
use paho_mqtt as mqtt;
use std::time::Duration;
use std::process;
use futures::sync::mpsc;
use meter::UsageData;
use std::thread;

#[derive(Debug)]
pub enum Error {
    DataStream,
    MqttPublish(mqtt::MqttError),
    MqttConnect(mqtt::MqttError)
}

pub fn run(data_stream: mpsc::Receiver<UsageData>) {
    thread::spawn(move || {
        let host = "tcp://mqtt:1883";
        let cli = mqtt::AsyncClientBuilder::new()
            .client_id(env!("CARGO_PKG_NAME"))
            .persistence(false)
            .offline_buffering(false)
            .server_uri(host)
            .finalize();

        let connect_options = mqtt::ConnectOptionsBuilder::new()
            .clean_session(true)
            .keep_alive_interval(Duration::from_secs(15*60))
            .will_message(mqtt::Message::new_retained("P1/availability", "offline", 1))
            .connect_timeout(Duration::from_secs(2*60))
            .automatic_reconnect(Duration::from_secs(10), Duration::from_secs(60*5))
            .finalize();

        cli.connect(connect_options).map_err(Error::MqttConnect)
            .and_then(move |_| {
                println!("MQTT Connected");
                cli.publish(mqtt::Message::new_retained("P1/availability", "online", 1)).map(|_| cli).map_err(Error::MqttPublish)
            })
            .and_then(move |cli| {
                data_stream.map_err(|()| Error::DataStream).for_each(move |ud| {
                    cli.publish(mqtt::Message::new("P1/power_delivered", format!("{}", ud.power_delivered), 0))
                        .join5(
                            cli.publish(mqtt::Message::new("P1/power_delivered_l1", format!("{}", ud.power_delivered_l1), 0)),
                            cli.publish(mqtt::Message::new("P1/power_delivered_l2", format!("{}", ud.power_delivered_l2), 0)),
                            cli.publish(mqtt::Message::new("P1/power_delivered_l3", format!("{}", ud.power_delivered_l3), 0)),
                            cli.publish(mqtt::Message::new("P1/gas_delivered", format!("{}", ud.gas_delivered), 0))
                        )
                        .join5(
                            cli.publish(mqtt::Message::new("P1/energy_delivered_tariff1", format!("{}", ud.energy_delivered_tariff1), 0)),
                            cli.publish(mqtt::Message::new("P1/energy_delivered_tariff2", format!("{}", ud.energy_delivered_tariff2), 0)),
                            cli.publish(mqtt::Message::new("P1/voltage_l1", format!("{}", ud.voltage_l1), 0)),
                            cli.publish(mqtt::Message::new("P1/voltage_l2", format!("{}", ud.voltage_l2), 0))
                        ).join(
                            cli.publish(mqtt::Message::new("P1/voltage_l3", format!("{}", ud.voltage_l3), 0))
                        )
                        .map(|_| ()).map_err(Error::MqttPublish)
                })
            })
            .wait()
            .map_err(|e| {
                eprintln!("Mqtt error: {:?}", e);
                process::exit(2);
            })
    });
}
