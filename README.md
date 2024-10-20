# RustCSRFTutorial
Rust project configuring the CSRF token with an in-memory session

## Instructions
How to test the code for CSRF protection:

1- Open Postman

2- Launch this application with ('cargo build' first if it's the first time, then) 'cargo run'

3- In Postman, type the address 127.0.0.1/csrf-token, and choose the GET method. This will return your csrf-token

4- In Postman, type the address 127.0.0.1/process, and choose the POST method. Add a new header from the headers section
called 'X-CSRF-Token' (without the single quotes) and put the value returned by the GET request at step 3 as it's associated value.
Then, execute the POST request. You have 30 seconds to do so until the CSRF token is invalidated.

5- To test for the CSRF token invalidation, wait more than 30 seconds until calling the POST method again, and you should get 'Invalid CSRF token'
(or 'Session timed out' if it hasn't been removed yet by the cleanup_sessions method)