mod location;
mod icloud;

use std::{env, time};
use std::sync::{Arc, Mutex};
use axum::extract::State;
use axum::response::Html;
use axum::Router;
use axum::routing::get;
use chrono::{Datelike, NaiveDate, Utc};
use rand::random;
use crate::icloud::{ICloudSession, ICloudSessionOps};
use crate::location::{get_location_text, LocationState, update_location};
use dotenv::dotenv;
use tokio::join;
use chrono_tz::US::Eastern;


fn base_html(title: String, inner: String) -> Html<String> {
    Html(format!(r#"
    <!DOCTYPE html><html>
    <head>
    <title>{title}</title>
    <style>{css}</style>
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    </head>
    <body>{inner}</body>
    </head></html>"#, css = include_str!("include/main.css")))
}

async fn index(State(state): State<AppState>) -> Html<String> {
    let greetings = [
        "Hello", "Hi", "Hey", "Howdy"
    ];
    let greeting =
        greetings[(random::<f64>() * greetings.len() as f64).floor() as usize];

    let location_state =
        state.location_state.lock().expect("not poisoned lock");

    let location_text = get_location_text(location_state.as_ref());

    let year = Utc::now().date_naive().year();

    let mut favorite_artists = vec![
        "Taylor Swift",
        "Hozier",
        "Conan Grey",
        "Baby Queen",
        "Shostakovich",
        "Charli XCX",
        "Mitski",
        "Panic! At The Disco",
        "Lorde",
        "SOPHIE",
    ];
    let mut random_artist = || favorite_artists.remove(
            (random::<f64>() * favorite_artists.len() as f64).floor() as usize);

    let random_artist_1 = random_artist();
    let random_artist_2 = random_artist();

    let time_ago = Utc::now().with_timezone(&Eastern).date_naive() -
        NaiveDate::from_ymd_opt(2024, 09, 12).expect("a valid date");
    let time_ago = match time_ago {
        _ if time_ago == chrono::Duration::days(0) => "today".to_string(),
        _ if time_ago == chrono::Duration::days(1) =>  "yesterday".to_string(),
        _ => time_ago.num_days().to_string() + " days ago"
    };

    base_html("Cheru Berhanu".to_string(),
                   format!(
                       include_str!("include/index.html"),
                       greeting = greeting,
                       location_text = location_text,
                       random_artist_1 = random_artist_1, random_artist_2 = random_artist_2,
                       year = year,
                       time_ago = time_ago
                   ))
}

#[derive(Clone)]
struct AppState {
    location_state: Arc<Mutex<Option<LocationState>>>,
    icloud_session: Arc<Mutex<ICloudSession>>,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let state = AppState {
        location_state: Arc::new(Mutex::new(None)),
        icloud_session: Arc::new(Mutex::new(ICloudSession::new(
            env::var("ICLOUD_EMAIL").expect("icloud email"),
            env::var("ICLOUD_PASSWORD").expect("icloud password")
        ).await.expect("icloud session")
        )),
    };

    let server_task = {
        let app = Router::new()
            .route("/", get(index))
            .with_state(state.clone());

        let port = env::var("PORT")
            .map_or_else(|_| "3000".to_string(), |x| x);

        let listener =
            tokio::net::TcpListener::bind("0.0.0.0:".to_string() + &port).await.unwrap();

        axum::serve(listener, app)
    };

    let location_update_task = async {
        let mut interval = tokio::time::interval(time::Duration::from_secs(60 * 60));

        loop {
            interval.tick().await;

            let state = state.clone();
            let _ = update_location(state.icloud_session, state.location_state).await
                .or_else(|_| { eprintln!("Updating location failed!"); Err(()) } );
        }
    };

    let _ = join!(server_task, location_update_task);
}