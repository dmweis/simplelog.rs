use rumqtt::{MqttClient, MqttOptions, QoS, ReconnectOptions};
// use std::sync::mpsc::{channel, Sender};
use crossbeam_channel::{unbounded, Sender};
use std::thread;
use std::io::Cursor;
use std::str;
use super::logging::try_log;
use crate::{Config, SharedLogger};
use log::{
    set_boxed_logger, set_max_level, LevelFilter, Log, Metadata, Record, SetLoggerError,
};

enum Command {
    SendMessage(String),
    Exit,
}

/// Logger that sends all data as mqtt messages
/// Mqtt client is running in a separate thread but this still isn't the fastest logger
/// Set logging level accordingly
pub struct MqttLogger {
    log_level: LevelFilter,
    config: Config,
    sender: Sender<Command>,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl MqttLogger {
    /// init function. Globally initializes the MqttLogger as the one and only used log facility.
    ///
    /// Takes the desired `Level` and `Config` as arguments.
    /// `host` - mqtt server hostname
    /// `application_name` - name under which application should show up
    /// 
    /// Logs will be published as `logging/{application_name}`
    /// 
    /// # Examples
    /// ```
    /// # extern crate simplelog;
    /// # use simplelog::*;
    /// # fn main() {
    /// let _ = MqttLogger::init(LevelFilter::Info, Config::default(), "mqtt.local", "application");
    /// # }
    /// ```
    pub fn init(log_level: LevelFilter, config: Config, host: &str, application_name: &str) -> Result<(), SetLoggerError> {
        set_max_level(log_level.clone());
        set_boxed_logger(MqttLogger::new(log_level, config, host, application_name))
    }

    /// Create new mqtt logger, that can be independently used, no matter what is globally set.
    ///
    /// no macros are provided for this case and you probably
    /// don't want to use this function, but `init()`, if you don't want to build a `CombinedLogger`.
    ///
    /// Takes the desired `Level` and `Config` as arguments.
    /// `host` - mqtt server hostname
    /// `application_name` - name under which application should show up
    /// 
    /// Logs will be published as `logging/{application_name}`
    /// 
    /// They cannot be changed later.
    ///
    /// # Examples
    /// ```
    /// # extern crate simplelog;
    /// # use simplelog::*;
    /// # fn main() {
    /// let simple_logger = MqttLogger::new(LevelFilter::Info, Config::default(), "mqtt.local", "application");
    /// # }
    /// ```
    pub fn new(log_level: LevelFilter, config: Config, host: &str, application_name: &str) -> Box<MqttLogger> {
        let host = host.to_owned();
        let application_name = application_name.to_owned();
        let (tx, rx): (Sender<Command>, _) = unbounded();
        let mqtt_thread = thread::spawn(move || {
            let mqtt_options = MqttOptions::new(application_name.clone(), host, 1883)
                .set_reconnect_opts(ReconnectOptions::Always(1));
            let (mut mqtt_client, _) = match MqttClient::start(mqtt_options) {
                Ok(client) => {
                    client
                },
                Err(_) => {
                    return;
                }
            };

            for message in rx.iter() {
                match message {
                    Command::SendMessage(message) => {
                        if let Err(_) =
                            mqtt_client.publish(format!("logging/{}", application_name), QoS::AtLeastOnce, false, message)
                        {
                            panic!("Failed to send log");
                        }
                    }
                    Command::Exit => {
                        break
                    },
                }
            }
        });
        Box::new(MqttLogger {
            log_level,
            config,
            sender: tx,
            join_handle: Some(mqtt_thread),
        })
    }

    fn send(&self, message: &str) {
        if let Err(_) = self.sender.send(Command::SendMessage(message.to_owned())) {
            panic!("Failed to send log");
        }
    }
}


impl Drop for MqttLogger {
    fn drop(&mut self) {
        if let Err(error) = self.sender.send(Command::Exit) {
            panic!("Failed to load Exit message to channel {}", error);
        }
        match self.join_handle.take() {
            Some(handle) => {
                if let Err(error) = handle.join() {
                    panic!("Failed joining MQTT thread with {:?}", error);
                }
            }
            None => panic!("Missing join handle for MQTT thread"),
        }
    }
}


impl Log for MqttLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.log_level
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            let mut data = vec![];
            let mut buffer = Cursor::new(&mut data);
            if try_log(&self.config, record, &mut buffer).is_ok() {
                if !data.is_empty() {
                    self.send(&str::from_utf8(&data).unwrap().trim_end());
                }
            }
        }
    }

    fn flush(&self) {}
}

impl SharedLogger for MqttLogger {
    fn level(&self) -> LevelFilter {
        self.log_level
    }

    fn config(&self) -> Option<&Config> {
        Some(&self.config)
    }

    fn as_log(self: Box<Self>) -> Box<dyn Log> {
        Box::new(*self)
    }
}
