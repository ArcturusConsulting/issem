#!/usr/bin/env python3
import os
import sys
import time
from cryptography.hazmat.primitives import serialization
import jwt

# 1. Require a client identifier argument
if len(sys.argv) < 2:
    print("❌ Error: Missing client identifier.")
    print("Usage: python3 gen_license.py <client-name>")
    sys.argv = [sys.argv[0], input("Enter client name to proceed: ").strip().lower().replace(" ", "-")]

CLIENT_NAME = sys.argv[1]
BASE_DIR = os.path.expanduser("~/issem")

# Master Key Location (Matches the openssl command you ran!)
MASTER_PRIVATE_KEY_PATH = os.path.join(BASE_DIR, "private_keys", "master_private.pem")
DIST_DIR = os.path.join(BASE_DIR, "vault", CLIENT_NAME)
os.makedirs(DIST_DIR, exist_ok=True)

# ─── STRICT GUARDRAIL: Read-Only Check ───
if not os.path.exists(MASTER_PRIVATE_KEY_PATH):
    print("❌" * 30)
    print("🚨 CRITICAL FAULT: Master Private Key Not Found!")
    print(f"Expected file at: {MASTER_PRIVATE_KEY_PATH}")
    print("\nPlease restore 'master_private.pem' from your secure backup.")
    print("❌" * 30)
    sys.exit(1)

print("🔑 Loading existing Master Private Key...")
with open(MASTER_PRIVATE_KEY_PATH, "rb") as f:
    private_key = serialization.load_pem_private_key(f.read(), password=None)

# Derive the matching public key
public_key = private_key.public_key()
public_pem = public_key.public_bytes(
    encoding=serialization.Encoding.PEM,
    format=serialization.PublicFormat.SubjectPublicKeyInfo
)

# Save public key copy to client directory (to satisfy your active K3s volume mounts)
with open(os.path.join(DIST_DIR, "public.pem"), "wb") as f:
    f.write(public_pem)

# 2. Assemble the cryptographic claims
payload = {
    "iss": "ArcturusConsulting",
    "sub": CLIENT_NAME,
    "iat": int(time.time()),
    "exp": int(time.time()) + (365 * 24 * 60 * 60), # 1 year validity
    "max_amr_fleet": 5
}

# 3. Sign using EdDSA
token = jwt.encode(payload, private_key, algorithm="EdDSA")
with open(os.path.join(DIST_DIR, "license.jwt"), "w") as f:
    f.write(token)

print(f"\n🛡️  Generated assets for client: [{CLIENT_NAME}]")
print(f"📂 Saved to: {DIST_DIR}")