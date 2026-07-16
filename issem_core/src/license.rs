use std::error::Error;
use std::fs::File;
use std::io::Read;
use log::{info, error, warn};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::Deserialize;

/// Compile-time check: Embeds your Master Public Key directly into the Rust binary.
/// Relative path resolution points to: ~/issem/pki/public.pem
const MASTER_PUBLIC_KEY: &[u8] = include_bytes!("../../pki/public.pem");

/// The structural layout of your commercial software license payload
#[derive(Debug, Deserialize, Clone)]
pub struct LicenseClaims {
    pub iss: String,          // Issuer (e.g., "ArcturusConsulting")
    pub sub: String,          // Customer Name
    pub exp: i64,             // Expiration timestamp
    pub max_amr_fleet: usize, // Hard ceiling for authorized robot counts
}

/// Evaluates the license file on the disk against the baked-in Master Public Key.
pub fn validate_startup_license() -> Result<LicenseClaims, Box<dyn Error + Send + Sync>> {
    info!("🔑 Validating production runtime activation license token...");
    
    // 1. Read the license file from the disk (clients can still mount their specific JWT)
    let mut license_file = File::open("license.jwt").map_err(|_| -> Box<dyn Error + Send + Sync> {
        "Licensing Fault: Cryptographic token validation failed! Missing required 'license.jwt' registration block.".into()
    })?;
    
    let mut jwt_token = String::new();
    license_file.read_to_string(&mut jwt_token)
        .map_err(|e| -> Box<dyn Error + Send + Sync> { 
            format!("Failed to read license file payload: {}", e).into() 
        })?;
    let jwt_token = jwt_token.trim();

    // 2. Decode using our statically embedded Master Public Key
    let decoding_key = DecodingKey::from_ed_pem(MASTER_PUBLIC_KEY)
        .map_err(|e| -> Box<dyn Error + Send + Sync> { 
            format!("Cryptographic validation fault: Embedded Master Public Key is malformed: {}", e).into() 
        })?;
        
    let mut validation = Validation::new(Algorithm::EdDSA);
    validation.validate_exp = true; 

    // 3. Verify the signature and unpack claims
    let token_data = decode::<LicenseClaims>(jwt_token, &decoding_key, &validation)
        .map_err(|err| -> Box<dyn Error + Send + Sync> {
            warn!("❌ Cryptographic Signature Check Failed: License altered or signature invalid.");
            format!("JWT Signature validation failed: {}", err).into()
        })?;

    let claims = token_data.claims;
    info!("✅ License verified successfully for client asset: [{}]. Issued by: {}. Expiry timestamp: {}", claims.sub, claims.iss, claims.exp);
    
    Ok(claims)
}

/// Spawns a resilient background supervisor that continually watches the system clock and license.
pub fn spawn_background_validator() {
    tokio::spawn(async move {
        let check_interval = std::time::Duration::from_secs(3600); 
        let mut consecutive_failures = 0;
        const MAX_CONSECUTIVE_FAILURES: u32 = 12; 

        // Extract decoding key once at thread startup
        let dec_key = DecodingKey::from_ed_pem(MASTER_PUBLIC_KEY)
            .expect("Embedded Master Public Key failed initialization");

        loop {
            tokio::time::sleep(check_interval).await;

            // Continually verify the client's mounted token against our embedded master key
            match std::fs::read_to_string("license.jwt") {
                Ok(fresh_jwt) => {
                    let val = Validation::new(Algorithm::EdDSA);
                    
                    if decode::<LicenseClaims>(fresh_jwt.trim(), &dec_key, &val).is_ok() {
                        consecutive_failures = 0;
                        continue;
                    }
                    consecutive_failures += 1;
                }
                Err(_) => {
                    consecutive_failures += 1;
                }
            }

            if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                error!("🚨 CRITICAL LICENSING BREACH: License expired or altered at runtime. Consecutive check failures exceeded threshold. Halting services!");
                std::process::exit(1); 
            } else if consecutive_failures > 0 {
                warn!("⚠️ Warning: Background license validation failing ({} of {}). Retrying next hour...", consecutive_failures, MAX_CONSECUTIVE_FAILURES);
            }
        }
    });
}