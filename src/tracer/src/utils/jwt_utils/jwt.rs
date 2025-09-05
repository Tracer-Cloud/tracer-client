use crate::utils::jwt_utils::claims::Claims;
use crate::utils::jwt_utils::clerk::ClerkJwtVerifier;

/// this function checks if the jwt is valid,
/// we will check for now only if the token is in the right format, has all the fields, the user_id (sub) is not null
/// and the token is not expired
pub async fn is_jwt_valid(token: &str) -> (bool, Option<Claims>) {
    let clerk_jwt_verifier: ClerkJwtVerifier = ClerkJwtVerifier::new();

    let verification_result = clerk_jwt_verifier.verify_token(token).await;

    match verification_result {
        Ok(claims) => {
            if claims.sub.is_empty() {
                eprintln!("Error validating jwt token from clerk: user ID not found");
                (false, None)
            } else {
                println!("Logged In successfully! Run `tracer init` to start collecting data");
                (true, Some(claims))
            }
        }
        Err(err) => {
            eprintln!("Error validating jwt token from clerk: {}", err);
            (false, None)
        }
    }
}
