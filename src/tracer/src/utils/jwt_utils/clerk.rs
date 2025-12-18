use crate::constants::{
    CLERK_ISSUER_DOMAIN_DEV, CLERK_ISSUER_DOMAIN_PROD, CLERK_JWKS_DOMAIN_DEV,
    CLERK_JWKS_DOMAIN_PROD,
};
use crate::utils::env::is_development_environment;
use crate::utils::jwt_utils::claims::Claims;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

pub struct ClerkJwtVerifier {
    jwks_url: String,
    issuer: String,
}

impl ClerkJwtVerifier {
    pub fn new(platform: &str) -> Self {
        let jwks_url: String;
        let issuer: String;

        // by default, the platform is "default" so we use add a check on is_development_environment() to get the right domain
        // if default + dev environment, we use the dev domain, if default + prod (not dev) environment, we use the prod domain
        if platform.eq_ignore_ascii_case("dev")
            || platform.eq_ignore_ascii_case("local")
            || (platform.eq_ignore_ascii_case("default") && is_development_environment())
        {
            jwks_url = CLERK_JWKS_DOMAIN_DEV.to_string();
            issuer = CLERK_ISSUER_DOMAIN_DEV.to_string();
        } else {
            jwks_url = CLERK_JWKS_DOMAIN_PROD.to_string();
            issuer = CLERK_ISSUER_DOMAIN_PROD.to_string();
        }

        Self { jwks_url, issuer }
    }

    /// Fetch JWKS from Clerk
    /// We don't use caching for now because there is a lot of change of tokens between dev and prod and we might use the
    /// wrong platform cached to decode the token, but i think it'll be added after
    pub async fn fetch_jwks(&self) -> Result<JwkSet, Box<dyn std::error::Error>> {
        let response = reqwest::get(&self.jwks_url).await?;
        let jwks: JwkSet = response.json().await?;

        Ok(jwks)
    }

    // Verify and decode JWT token
    pub async fn verify_token(&self, token: &str) -> Result<Claims, Box<dyn std::error::Error>> {
        // Get the token header to find the key ID
        let header = jsonwebtoken::decode_header(token)?;
        let key_id = header.kid.ok_or("Token missing key ID")?;

        // Get JWKS
        let jwks = self.fetch_jwks().await?;

        // Find the key with matching kid
        let key = jwks
            .keys
            .iter()
            .find(|k| k.common.key_id.as_ref() == Some(&key_id))
            .ok_or("Key not found in JWKS")?;

        // Create a decoding key
        let decoding_key = DecodingKey::from_jwk(key)?;

        // Set up validation
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);
        validation.validate_exp = true;

        // Decode token
        let token_data = decode::<Claims>(token, &decoding_key, &validation)?;

        Ok(token_data.claims)
    }
}
