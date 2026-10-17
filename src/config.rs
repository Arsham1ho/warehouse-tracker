/// Configuration is read once at startup and validated, so a missing or
/// malformed variable fails immediately with a clear message rather than
/// surfacing as a confusing error on the first request.
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub mqtt_host: String,
    pub mqtt_port: u16,
    pub mqtt_topic: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set (see .env.example)"))?,
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into()),
            mqtt_host: std::env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".into()),
            mqtt_port: std::env::var("MQTT_PORT")
                .ok()
                .map(|p| p.parse())
                .transpose()
                .map_err(|_| anyhow::anyhow!("MQTT_PORT must be a number"))?
                .unwrap_or(1883),
            mqtt_topic: std::env::var("MQTT_TOPIC").unwrap_or_else(|_| "warehouse/scans".into()),
        })
    }
}
