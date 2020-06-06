use log::*;
use simplelog::*;
use std::time::Duration;
use std::thread::sleep;


fn main() {
    let config = ConfigBuilder::new()
        .set_thread_level(LevelFilter::Error)
        .set_thread_mode(ThreadLogMode::Both)
        .add_filter_allow_str("mqtt_logger_example")
        .build();
    CombinedLogger::init(vec![
        #[cfg(feature = "termcolor")]
        TermLogger::new(LevelFilter::Warn, Config::default(), TerminalMode::Mixed),
        MqttLogger::new(LevelFilter::Info, config, "mqtt.local", "demo"),
    ])
    .unwrap();

    error!("Bright red error");
    info!("This only appears in the mqtt log");
    debug!("This level is currently not enabled for any logger");
    // Have to wait for all messages to be sent
    sleep(Duration::from_secs(5));
}
