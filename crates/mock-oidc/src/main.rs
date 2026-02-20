use axum::{
    extract::{Form, Query},
    response::{Json, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::OnceLock;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use chrono::{Utc, Duration};
use rsa::{RsaPrivateKey, RsaPublicKey, pkcs1::EncodeRsaPrivateKey};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};


// Global Keys
struct OidcKeys {
    encoding_key: EncodingKey,
    public_jwk: Value,
}

static KEYS: OnceLock<OidcKeys> = OnceLock::new();

#[tokio::main]
async fn main() {
    // 1. Generate RSA Key Pair on Startup
    println!("MOCK-OIDC: Generating RSA-2048 keys...");
    let mut rng = rand::thread_rng();
    let bits = 2048;
    let priv_key = RsaPrivateKey::new(&mut rng, bits).expect("Failed to generate private key");
    let pub_key = RsaPublicKey::from(&priv_key);

    // Convert to PEM for jsonwebtoken
    // jsonwebtoken EncodingKey::from_rsa_pem expects PKCS#1 or PKCS#8.
    let priv_pem = priv_key.to_pkcs1_pem(rsa::pkcs8::LineEnding::LF).unwrap();
    let encoding_key = EncodingKey::from_rsa_pem(priv_pem.as_bytes()).unwrap();

    // Construct JWK for the public key (Naive construction)
    // For proper JWK we need Modulus (n) and Exponent (e) in Base64URL
    use rsa::traits::PublicKeyParts;
    let n = base64_url_encode_bytes(&pub_key.n().to_bytes_be());
    let e = base64_url_encode_bytes(&pub_key.e().to_bytes_be());
    
    let public_jwk = json!({
        "kty": "RSA",
        "alg": "RS256",
        "use": "sig",
        "kid": "mock-key-1",
        "n": n,
        "e": e
    });

    KEYS.set(OidcKeys { encoding_key, public_jwk }).ok().unwrap();

    // 2. Setup Routes
    let app = Router::new()
        .route("/.well-known/openid-configuration", get(openid_configuration))
        .route("/jwks", get(jwks))
        .route("/authorize", get(authorize)) // Dummy
        .route("/token", post(token));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000").await.unwrap();
    println!("MOCK-OIDC: Listening on http://localhost:4000");
    axum::serve(listener, app).await.unwrap();
}

// --- Endpoints ---

async fn openid_configuration() -> Json<Value> {
    Json(json!({
        "issuer": "http://localhost:4000",
        "authorization_endpoint": "http://localhost:4000/authorize",
        "token_endpoint": "http://localhost:4000/token",
        "jwks_uri": "http://localhost:4000/jwks",
        "response_types_supported": ["code", "token", "id_token"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"]
    }))
}

async fn jwks() -> Json<Value> {
    let keys = KEYS.get().unwrap();
    Json(json!({
        "keys": [keys.public_jwk.clone()]
    }))
}

#[derive(Deserialize)]
struct AuthorizeParams {
    client_id: Option<String>,
    redirect_uri: String,
    state: Option<String>,
    response_type: Option<String>,
}

async fn authorize(Query(params): Query<AuthorizeParams>) -> impl IntoResponse {
    println!("MOCK-OIDC: Authorize request for client_id={:?}", params.client_id);
    
    // Auto-approve: Redirect strictly to the provided redirect_uri with a fixed code
    // In a real app, we would show a login page here.
    let code = "PILOT"; // Default to PILOT role for generic testing
    let state = params.state.unwrap_or_default();
    
    // Check if redirect_uri already has params (basic append)
    let separator = if params.redirect_uri.contains('?') { '&' } else { '?' };
    let target = format!("{}{}{}code={}&state={}", params.redirect_uri, separator, if separator == '&' { "" } else { "" }, code, state);
    
    Redirect::to(&target)
}

#[derive(Deserialize)]
struct TokenRequest {
    code: String,
    // grant_type: String, // optional here to be lenient
}

#[derive(Serialize)]
struct IdTokenClaims {
    iss: String,
    sub: String,
    aud: String,
    exp: i64,
    iat: i64,
    // Custom claims
    name: String,
    email: String,
    demonetwork_cid: String, 
}

async fn token(Form(req): Form<TokenRequest>) -> Json<Value> {
    println!("MOCK-OIDC: Token Request code='{}'", req.code);

    // Map Code to Identity
    let (sub, name, email) = match req.code.as_str() {
        "PILOT" => ("100000".to_string(), "Captain Smith".to_string(), "pilot@demonetwork.net".to_string()),
        "ATC" => ("888888".to_string(), "Generic ATC".to_string(), "atc@demonetwork.net".to_string()),
        "ATC_EGLL" => ("777777".to_string(), "Generic ATC".to_string(), "atc@demonetwork.net".to_string()),

        // Dynamic Fallback: use the provided code as the identity
        // This allows the CLI to request tokens for any network address
        other => (other.to_string(), format!("User {}", other), format!("{}@demonetwork.net", other.to_lowercase())),
    };

    let now = Utc::now();
    let exp = now + Duration::hours(1);

    let claims = IdTokenClaims {
        iss: "http://localhost:4000".to_string(),
        sub: sub.to_string(),
        aud: "openlink-auth".to_string(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
        name: name.to_string(),
        email: email.to_string(),
        demonetwork_cid: sub.to_string(), 
    };

    let keys = KEYS.get().unwrap();
    let header = Header {
        kid: Some("mock-key-1".to_string()),
        alg: Algorithm::RS256,
        ..Default::default()
    };

    let id_token = encode(&header, &claims, &keys.encoding_key).unwrap();
    
    // Access token format consumed by openlink-auth
    let access_token = format!("demonetwork_{}", sub); 

    Json(json!({
        "access_token": access_token, 
        "id_token": id_token,
        "token_type": "Bearer",
        "expires_in": 3600
    }))
}

fn base64_url_encode_bytes(input: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(input)
}
