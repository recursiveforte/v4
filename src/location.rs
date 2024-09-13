use std::{env, mem};
use std::cmp::max;
use std::error::Error;
use std::sync::{Arc, Mutex};
use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use isocountry::CountryCode;
use crate::icloud::{ICloudSession, ICloudSessionOps};

struct Location {
    name: &'static str,
    state: &'static str,
    country: CountryCode,
    lat: f64,
    long: f64
}

const CITIES:
[&Location; include!(concat!(env!("OUT_DIR"), "/cities_len.in"))] =
    include!(concat!(env!("OUT_DIR"), "/cities.in"));

const STATES: [(&str, &str); 62] = include!("include/states.rs.in");

fn get_state(abbr: &str) -> Result<&str, Box<dyn Error>> {
    for state in STATES {
        if state.0 == abbr {
            return Ok(state.1)
        }
    };
    Err("State not found.".into())
}

pub fn find_nearest_city(lat: f64, long: f64) -> Result<String, Box<dyn Error>> {
    let mut closest_distance = -1_f64;
    let mut closest_city = String::new();

    let location = geoutils::Location::new(lat, long);

    for city in CITIES {
        let dist = geoutils::Location::new(city.lat, city.long).haversine_distance_to(&location).meters();

        if dist < closest_distance || closest_distance == -1_f64 {
            closest_distance = dist;
            closest_city = city.name.to_owned() + ", " + if city.country == CountryCode::USA {
                get_state(city.state)?
            } else {
                city.country.name()
            }
        }
    }

    return Ok(closest_city);
}

pub struct LocationState {
    last_updated_time: DateTime<Utc>,
    closest_city: String,
}

pub async fn update_location(icloud_session: Arc<Mutex<ICloudSession>>,
                         location_state: Arc<Mutex<Option<LocationState>>>)
                         -> Result<(), Box<dyn Error>> {
    let locations =
        icloud_session.lock().expect("lock not to fail").get_locations().await?;

    let device_name = env::var("DEVICE_NAME").expect("findmy device name");

    for device in locations {
        if device.name == device_name {
            let location = device.location.ok_or("location not found")?;

            let new_location_state = Some(LocationState {
                last_updated_time: Utc.timestamp_millis_opt(location.timestamp)
                    .single().ok_or("invalid timestamp")?,
                closest_city: find_nearest_city(location.latitude, location.longitude)?
            });

            let mut location_state =
                location_state.lock().expect("lock not to fail");

            let _ = mem::replace(&mut *location_state,  new_location_state);

            return Ok(())
        }
    }

    Err("device not found".into())
}

pub fn get_location_text(location_state: Option<&LocationState>) -> String {
    let default_location_text = "I'm based in Burlington, Vermont".to_string();

    if let Some(state) = location_state {
        let time_since_update = max(Utc::now() - state.last_updated_time, 
                                    TimeDelta::new(0, 0)
                                        .expect("time delta to succeed"));
        let (time_since_update, unit) =
            if time_since_update.num_minutes() < 60 {
                (time_since_update.num_minutes(), "minutes")
            } else if time_since_update.num_hours() < 24 {
                (time_since_update.num_hours(), "hours")
            } else {
                (time_since_update.num_days(), "days")
            };

        let unit = if time_since_update != 1 { unit } else { &unit[..unit.len() - 1] };

        if time_since_update > 99 {
            default_location_text
        } else if time_since_update == 0 {
            format!("as of now, I'm in {}", state.closest_city)
        } else {
            format!("as of {time_since_update} {unit} ago, I'm in {}", state.closest_city)
        }
    } else { default_location_text }
}