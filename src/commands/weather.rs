use super::Replies;
use crate::db::User;
use crate::{linefeed, BBSConfig, WeatherConfig};
use diesel::SqliteConnection;
use serde::Deserialize;
use std::time::Duration;
use ureq::{Agent, Error as UreqError};
use url::Url;

const DEFAULT_API_BASE: &str = "https://api.open-meteo.com/v1/forecast";
const WEATHER_NOT_CONFIGURED: &str = "Weather information is not configured for this node.";
const WEATHER_UNAVAILABLE: &str = "Unable to retrieve the weather right now.";

#[derive(Debug)]
enum WeatherError {
    InvalidBase(String),
    Request(String),
    MissingData,
    Status(u16),
}

#[derive(Debug, Deserialize)]
struct WeatherApiResponse {
    timezone: Option<String>,
    current_weather: Option<CurrentWeather>,
}

#[derive(Debug, Deserialize)]
struct CurrentWeather {
    temperature: f64,
    windspeed: f64,
    #[serde(default)]
    winddirection: Option<f64>,
    weathercode: i32,
    time: String,
}

struct WeatherReport {
    current: CurrentWeather,
    timezone: Option<String>,
}

pub fn current(
    _conn: &mut SqliteConnection,
    cfg: &BBSConfig,
    _user: &mut User,
    _args: Vec<&str>,
) -> Replies {
    let Some(weather_cfg) = &cfg.weather else {
        return WEATHER_NOT_CONFIGURED.into();
    };

    match fetch_weather(weather_cfg) {
        Ok(report) => format_report(weather_cfg, &report).into(),
        Err(err) => {
            log_weather_error(&err);
            WEATHER_UNAVAILABLE.into()
        }
    }
}

fn fetch_weather(config: &WeatherConfig) -> Result<WeatherReport, WeatherError> {
    let base = config.api_base.as_deref().unwrap_or(DEFAULT_API_BASE);
    let mut url = Url::parse(base).map_err(|err| WeatherError::InvalidBase(err.to_string()))?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("latitude", &format!("{:.6}", config.latitude));
        pairs.append_pair("longitude", &format!("{:.6}", config.longitude));
        pairs.append_pair("current_weather", "true");
    }

    let user_agent = format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    let agent: Agent = Agent::new_with_config(
        Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .user_agent(user_agent)
            .build(),
    );

    let mut response = agent.get(url.as_str()).call().map_err(|err| match err {
        UreqError::StatusCode(code) => WeatherError::Status(code),
        other => WeatherError::Request(other.to_string()),
    })?;

    let payload: WeatherApiResponse = response
        .body_mut()
        .read_json()
        .map_err(|err| WeatherError::Request(err.to_string()))?;
    let WeatherApiResponse {
        timezone,
        current_weather,
    } = payload;
    let current = current_weather.ok_or(WeatherError::MissingData)?;

    Ok(WeatherReport { current, timezone })
}

fn format_report(config: &WeatherConfig, report: &WeatherReport) -> Vec<String> {
    let mut out = Vec::new();
    let label = config
        .location_name
        .clone()
        .unwrap_or_else(|| format!("{:.4}, {:.4}", config.latitude, config.longitude));
    out.push(format!("Weather for {label}"));
    linefeed!(out);

    let tz = report
        .timezone
        .as_deref()
        .filter(|tz| !tz.is_empty())
        .map(|tz| format!(" {tz}"))
        .unwrap_or_default();
    out.push(format!("Observed at {}{}", report.current.time, tz));
    out.push(format!(
        "Conditions: {}",
        describe_weather_code(report.current.weathercode)
    ));
    out.push(format!(
        "Temperature: {:.1}°C ({:.1}°F)",
        report.current.temperature,
        c_to_f(report.current.temperature)
    ));

    let wind = format!(
        "Wind: {:.1} km/h ({:.1} mph)",
        report.current.windspeed,
        kmh_to_mph(report.current.windspeed)
    );
    if let Some(direction) = report.current.winddirection {
        out.push(format!(
            "{wind} from {}° {}",
            direction.round(),
            cardinal_direction(direction)
        ));
    } else {
        out.push(wind);
    }

    out
}

fn describe_weather_code(code: i32) -> &'static str {
    match code {
        0 => "Clear sky",
        1 => "Mainly clear",
        2 => "Partly cloudy",
        3 => "Overcast",
        45 => "Fog",
        48 => "Depositing rime fog",
        51 => "Light drizzle",
        53 => "Moderate drizzle",
        55 => "Dense drizzle",
        56 => "Light freezing drizzle",
        57 => "Dense freezing drizzle",
        61 => "Slight rain",
        63 => "Moderate rain",
        65 => "Heavy rain",
        66 => "Light freezing rain",
        67 => "Heavy freezing rain",
        71 => "Slight snow fall",
        73 => "Moderate snow fall",
        75 => "Heavy snow fall",
        77 => "Snow grains",
        80 => "Slight rain showers",
        81 => "Moderate rain showers",
        82 => "Violent rain showers",
        85 => "Slight snow showers",
        86 => "Heavy snow showers",
        95 => "Thunderstorm",
        96 => "Thunderstorm with light hail",
        99 => "Thunderstorm with heavy hail",
        _ => "Unknown conditions",
    }
}

fn c_to_f(celsius: f64) -> f64 {
    celsius * 9.0 / 5.0 + 32.0
}

fn kmh_to_mph(kmh: f64) -> f64 {
    kmh / 1.609_344
}

fn cardinal_direction(degrees: f64) -> &'static str {
    const POINTS: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    let mut deg = degrees % 360.0;
    if deg < 0.0 {
        deg += 360.0;
    }
    #[allow(clippy::cast_possible_truncation)]
    let index = ((deg / 22.5).round() as usize) % POINTS.len();
    POINTS[index]
}

fn log_weather_error(err: &WeatherError) {
    match err {
        WeatherError::InvalidBase(details) => {
            log::error!("Weather fetch failed: invalid API base: {details}");
        }
        WeatherError::Request(inner) => log::error!("Weather fetch failed: {inner}"),
        WeatherError::MissingData => {
            log::error!("Weather fetch failed: missing current weather data");
        }
        WeatherError::Status(code) => {
            log::error!("Weather fetch failed: HTTP status {code}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, users};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn test_config() -> BBSConfig {
        BBSConfig {
            bbs_name: "Test BBS".to_string(),
            my_id: "!00000001".to_string(),
            db_path: ":memory:".to_string(),
            serial_device: None,
            tcp_address: None,
            sysops: vec![],
            public_channel: 0,
            ad_text: "".to_string(),
            weather: None,
            menus: config::Map::new(),
        }
    }

    #[test]
    fn directions_make_sense_around_0_degress() {
        // Just slightly more NNW than N
        assert_eq!(cardinal_direction(-11.26), "NNW");
        // Within the 360/16==22.5º wide window from -11.25.º to +11.25º
        assert_eq!(cardinal_direction(-11.25), "N");
        // Due north
        assert_eq!(cardinal_direction(0.0), "N");
        // Just slightly more N than NNE
        assert_eq!(cardinal_direction(11.249), "N");
        // Over the threshold of being more NNE
        assert_eq!(cardinal_direction(11.25), "NNE");
    }

    #[test]
    fn weather_requires_configuration() {
        let cfg = test_config();
        let mut conn = db::test_connection();
        let (mut user, _) = users::record(&mut conn, "!00000001").expect("user record");
        let replies = current(&mut conn, &cfg, &mut user, vec![]);
        assert_eq!(replies.0[0].out, vec![WEATHER_NOT_CONFIGURED.to_string()]);
    }

    #[test]
    fn weather_reports_conditions() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        listener.set_nonblocking(false).expect("blocking listener");
        let addr = listener.local_addr().unwrap();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer);
            let body = r#"{
  "timezone": "Etc/UTC",
  "current_weather": {
    "temperature": 12.3,
    "windspeed": 14.0,
    "winddirection": 180.0,
    "weathercode": 2,
    "time": "2025-02-15T12:00"
  }
}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            stream.write_all(response.as_bytes()).expect("write");
        });

        let mut cfg = test_config();
        cfg.weather = Some(WeatherConfig {
            latitude: 40.0,
            longitude: -75.0,
            location_name: Some("Testville".to_string()),
            api_base: Some(format!("http://{}/v1/forecast", addr)),
        });

        let mut conn = db::test_connection();
        let (mut user, _) = users::record(&mut conn, "!00000001").expect("user record");
        let replies = current(&mut conn, &cfg, &mut user, vec![]);
        handle.join().expect("server thread");

        let output = &replies.0[0].out;
        assert!(output
            .iter()
            .any(|line| line.contains("Weather for Testville")));
        assert!(output
            .iter()
            .any(|line| line.contains("Observed at 2025-02-15T12:00 Etc/UTC")));
        assert!(output.iter().any(|line| line.contains("Partly cloudy")));
        assert!(output.iter().any(|line| line.contains("12.3°C")));
        assert!(output.iter().any(|line| line.contains("from 180° S")));
    }
}
