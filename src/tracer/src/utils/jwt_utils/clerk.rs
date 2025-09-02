use crate::constants::{CLERK_ISSUER_DOMAIN, CLERK_JWKS_DOMAIN};
use crate::utils::jwt_utils::claims::Claims;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use once_cell::sync::Lazy;
use std::sync::RwLock;

// JWKS cache to avoid frequent network requests
static JWKS_CACHE: Lazy<RwLock<Option<JwkSet>>> = Lazy::new(|| RwLock::new(None));

pub struct ClerkJwtVerifier {
    jwks_url: String,
    issuer: String,
}

impl ClerkJwtVerifier {
    pub fn new() -> Self {
        Self {
            jwks_url: CLERK_JWKS_DOMAIN.to_string(),
            issuer: CLERK_ISSUER_DOMAIN.to_string(),
        }
    }

    // Fetch JWKS from Clerk
    pub async fn fetch_jwks(&self) -> Result<JwkSet, Box<dyn std::error::Error>> {
        let response = reqwest::get(&self.jwks_url).await?;
        let jwks: JwkSet = response.json().await?;

        // Cache the JWKS
        if let Ok(mut cache) = JWKS_CACHE.write() {
            *cache = Some(jwks.clone());
        }

        Ok(jwks)
    }

    // Get JWKS (from cache or fetch)
    async fn get_jwks(&self) -> Result<JwkSet, Box<dyn std::error::Error>> {
        // Try to get from cache first
        if let Ok(cache) = JWKS_CACHE.read() {
            if let Some(ref jwks) = *cache {
                return Ok(jwks.clone());
            }
        }

        // Fetch if not in cache
        self.fetch_jwks().await
    }

    // Verify and decode JWT token
    pub async fn verify_token(&self, token: &str) -> Result<Claims, Box<dyn std::error::Error>> {
        // Get the token header to find the key ID
        let header = jsonwebtoken::decode_header(token)?;
        let kid = header.kid.ok_or("Token missing key ID")?;

        // Get JWKS
        let jwks = self.get_jwks().await?;

        // Find the key with matching kid
        let key = jwks
            .keys
            .iter()
            .find(|k| k.common.key_id.as_ref() == Some(&kid))
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
