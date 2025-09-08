use crate::constants::JWT_TOKEN_FILE_PATH;
use crate::utils::jwt_utils::claims::Claims;
use crate::utils::jwt_utils::clerk::ClerkJwtVerifier;

/// this function checks if the jwt is valid,
/// we will check for now only if the token is in the right format, has all the fields, the user_id (sub) is not null
/// and the token is not expired
pub async fn is_jwt_valid(token: &str, platform: &str) -> (bool, Option<Claims>) {
    let clerk_jwt_verifier: ClerkJwtVerifier = ClerkJwtVerifier::new(platform);

    let verification_result = clerk_jwt_verifier.verify_token(token).await;

    match verification_result {
        Ok(claims) => {
            if claims.sub.is_empty() {
                eprintln!("Error validating jwt token from clerk: user ID not found. Try 'tracer login' again");
                (false, None)
            } else {
                (true, Some(claims))
            }
        }
        Err(err) => {
            eprintln!(
                "Error validating jwt token from clerk: {}, try 'tracer login' again",
                err
            );
            (false, None)
        }
    }
}

/// reads the file token.txt and returns the claims if the token is valid
pub async fn get_token_claims_from_file(platform: &str) -> Option<Claims> {
    // read the token.txt file
    let token = std::fs::read_to_string(JWT_TOKEN_FILE_PATH).ok()?;

    let is_token_valid_with_claims = is_jwt_valid(token.as_str(), platform).await;

    if is_token_valid_with_claims.0 {
        Some(is_token_valid_with_claims.1.unwrap())
    } else {
        None
    }
}
