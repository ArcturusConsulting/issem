#!/usr/bin/env python3
import os
import sys
import time
from cryptography.hazmat.primitives.asymmetric import ed25519
from cryptography.hazmat.primitives import serialization
import jwt

# 1. Enforce passing a client name via the terminal command line
if len(sys.argv) < 2:
    print("❌ Error: Missing client identifier.")
    print("Usage: python3 gen_license.py <client-name>")
    print("Example: python3 gen_license.py acme-logistics")
    sys.argv = [sys.argv[0], input("Enter client name to proceed: ").strip().lower().replace(" ", "-")]

CLIENT_NAME = sys.argv[1]

# 2. Dynamically route paths to an isolated vault directory for this client
BASE_DIR = os.path.expanduser("~/issem")
VAULT_DIR = os.path.join(BASE_DIR, "vault", CLIENT_NAME)
os.makedirs(VAULT_DIR, exist_ok=True)

print(f"🛡️ Generating isolated cryptographic assets for client: [{CLIENT_NAME}]")
private_key = ed25519.Ed25519PrivateKey.generate()
public_key = private_key.public_key()

# Save Public Key inside the client's folder
public_pem = public_key.public_bytes(
    encoding=serialization.Encoding.PEM,
    format=serialization.PublicFormat.SubjectPublicKeyInfo
)
with open(os.path.join(VAULT_DIR, "public.pem"), "wb") as f:
    f.write(public_pem)

# Save Private Key inside the client's folder (NEVER SHARE THIS)
private_pem = private_key.private_bytes(
    encoding=serialization.Encoding.PEM,
    format=serialization.PrivateFormat.PKCS8,  # ◄ CHANGED THIS LINE
    encryption_algorithm=serialization.NoEncryption()
)
with open(os.path.join(VAULT_DIR, "private.key"), "wb") as f:
    f.write(private_pem)

# Build the payload, explicitly tagging the client_id
payload = {
    "iss": "ArcturusConsulting",
    "sub": "issem-enterprise-runtime",
    "client_id": CLIENT_NAME,
    "iat": int(time.time()),
    "exp": int(time.time()) + (365 * 24 * 60 * 60)
}

# Encode and save the unique license
token = jwt.encode(payload, private_key, algorithm="EdDSA")
with open(os.path.join(VAULT_DIR, "license.jwt"), "w") as f:
    f.write(token)

print(f"✨ Success! Assets securely saved under: {VAULT_DIR}")