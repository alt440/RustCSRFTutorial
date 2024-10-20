use axum::{
    extract::Extension,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/*
How to test the code below:
1- Open Postman
2- Launch this application with ('cargo build' first if it's the first time, then) 'cargo run'
3- In Postman, type the address 127.0.0.1/csrf-token, and choose the GET method. This will return your csrf-token
4- In Postman, type the address 127.0.0.1/process, and choose the POST method. Add a new header from the headers section
called 'X-CSRF-Token' (without the single quotes) and put the value returned by the GET request at step 3 as it's associated value.
Then, execute the POST request. You have 30 seconds to do so until the CSRF token is invalidated.
5- To test for the CSRF token invalidation, wait more than 30 seconds until calling the POST method again, and you should get 'Invalid CSRF token'
(or 'Session timed out' if it hasn't been removed yet by the cleanup_sessions method)
*/

// 30 seconds timeout of CSRF token
const SECONDS_TIMEOUT: u64 = 30;
const SESSION_TIMEOUT: Duration = Duration::new(SECONDS_TIMEOUT, 0);

#[derive(Clone)]
struct AppState {
    sessions: Arc<Mutex<HashMap<String, Instant>>>, // In-memory session store
}

#[tokio::main]
async fn main() {
    // Create a session store. Arc = Atomic Reference Counting. Safe sharing of data across multiple threads.
    // Arc only indicates that something will be shared, thus it is Thread Safe (it could be pretty much anything)
    // Any data structure creates simple in-memory store of sessions, thus nothing persisted to SSD / HDD
    // Anything other than memory store will need some custom implementation (File store, DB store, ...)
    // IMPORTANT: This can only work when you have 1 backend server. If you have multiple, I suggest to use Redis and handle
    // sessions through DB requests! Redis has TTL, which automatically expires sessions after a certain time.
    let app_state = Arc::new(AppState {
        sessions: Arc::new(Mutex::new(HashMap::new())),
    });
    
    // Build the router and its different paths
    let app = Router::new()
        // route to /csrf-token, http get with get_csrf_token function call
        .route("/csrf-token", get(get_csrf_token))
        // route to /process, http post with process_form function call
        .route("/process", post(check_csrf_token))
        // 'layer' adds a functionality for all requests here. --> 'add functionality' are keywords here
        // Extension means that you are adding a shared object across all requests --> 'shared object' are keywords here
        // this essentially means that we are adding the store object to be accessible across all requests
        .layer(Extension(app_state.clone()));


    // Start the cleanup task
    // You explicitely need to do a clone, because of Rust ownership principle. By Rust ownership principle,
    // you would have the reference to app_state disappear (as it is used in layer method above and here in this method)
    let cleanup_state = app_state.clone();
    // Creates the background task
    tokio::spawn(async move {
        cleanup_sessions(cleanup_state).await; //infinite loop, thus this will execute forever
    });    

    // Run the application on port 3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        // converts router object to MakeService (required by serve method)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// Generate a CSRF token
fn generate_csrf_token() -> String {
    let mut rng = rand::thread_rng();
    let token: u64 = rng.gen();
    token.to_string()
}

// Endpoint to get the CSRF token
async fn get_csrf_token(
    Extension(store): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    let token = generate_csrf_token();
    let token_clone: String = token.clone();
    let now = Instant::now();

    // Need to lock the mutex (always) before performing anything on it
    store.sessions.lock()
                  .expect("The store could not lock for some reason")
                  .insert(token, now);

    // Return the CSRF token in the response
    (StatusCode::OK, token_clone)
}

// Process form submissions
async fn check_csrf_token(
    Extension(store): Extension<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    // Validate CSRF token
    let request_token = headers.get("X-CSRF-Token").and_then(|h| h.to_str().ok()).map(|s| s.to_string());
    // transforms the request_token option to string and sets default value to "" if option = none
    let request_token_val: String = request_token.unwrap_or("".to_string());
    // Passing a reference, no cloning needed
    let is_stored_token: bool = store.sessions.lock()
                                              .expect("Could not lock mutex")
                                              .contains_key(&request_token_val);

    if !is_stored_token {
        return Err((StatusCode::FORBIDDEN, "Invalid CSRF token"));
    }

    // I initialize it here by default. At this point, we know the CSRF token is in the map, so last_activity
    // is not going to be the instant time
    let mut last_activity: Instant = Instant::now();
    // Passing a reference, no cloning needed
    if let Some(start_time_ref) = store.sessions.lock()
                                                          .expect("Could not lock mutex")
                                                          .get(&request_token_val) {
        // `start_time_ref` is of type `&Instant`
        last_activity = *start_time_ref; // Dereference to get the `Instant`
    }
    
    let current_time = Instant::now();

    // Check if the session is expired
    if current_time.duration_since(last_activity) > SESSION_TIMEOUT {
        return Err((StatusCode::UNAUTHORIZED, "Session expired"));
    }

    // Update last activity
    store.sessions.lock().expect("Could not lock mutex")
                         .insert(request_token_val, current_time);
    
    Ok(StatusCode::OK)
}


// You wouldn't need to have a background process if you had a DB that handles TTL
// AppState is the struct defined earlier
async fn cleanup_sessions(state: Arc<AppState>) {
    loop {
        {
            let mut sessions = state.sessions.lock().unwrap();
            let now = Instant::now();
            sessions.retain(|_, &mut last_activity| now.duration_since(last_activity) < SESSION_TIMEOUT);
        }
        tokio::time::sleep(SESSION_TIMEOUT).await; // Run every SESSION_TIMEOUT seconds
    }
}