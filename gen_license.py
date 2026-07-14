#!/usr/bin/env python3
import os
import sys
import time
from cryptography.hazmat.primitives.asymmetric import ed25519
from cryptography.hazmat.primitives import serialization
import jwt

# 1. Require a client identifier argument
if len(sys.argv) < 2:
    print("❌ Error: Missing client identifier.")
    print("Usage: python3 gen_license.py <client-name>")
    sys.argv = [sys.argv[0], input("Enter client name to proceed: ").strip().lower().replace(" ", "-")]

CLIENT_NAME = sys.argv[1]
BASE_DIR = os.path.expanduser("~/issem")

# ─── SECURE SEPARATION OF TARGET DIRECTORIES ───
# Path A: Secure developer-only vault (Hidden from Git, Docker, and Clients)
PRIVATE_KEYS_DIR = os.path.join(BASE_DIR, "private_keys")
os.makedirs(PRIVATE_KEYS_DIR, exist_ok=True)
os.chmod(PRIVATE_KEYS_DIR, 0o700) # Only you can read/write/enter

# Path B: Public client-facing vault (Safe to mount to containers and email out)
DIST_DIR = os.path.join(BASE_DIR, "vault", CLIENT_NAME)
os.makedirs(DIST_DIR, exist_ok=True)

print(f"🛡️ Generating isolated cryptographic assets for client: [{CLIENT_NAME}]")
private_key = ed25519.Ed25519PrivateKey.generate()
public_key = private_key.public_key()

# 2. Save the Public Verification Key to the CLIENT distribution folder
public_pem = public_key.public_bytes(
    encoding=serialization.Encoding.PEM,
    format=serialization.PublicFormat.SubjectPublicKeyInfo
)
with open(os.path.join(DIST_DIR, "public.pem"), "wb") as f:
    f.write(public_pem)

# 3. Save the Secret Private Signing Key to the SECURE developer folder
private_pem = private_key.private_bytes(
    encoding=serialization.Encoding.PEM,
    format=serialization.PrivateFormat.PKCS8,
    encryption_algorithm=serialization.NoEncryption()
)
private_key_path = os.path.join(PRIVATE_KEYS_DIR, f"{CLIENT_NAME}.key")
with open(private_key_path, "wb") as f:
    f.write(private_pem)
os.chmod(private_key_path, 0o600) # Only your OS user can read/write this file

# 4. Assemble the cryptographic contract claims
payload = {
    "iss": "ArcturusConsulting",
    "sub": CLIENT_NAME,
    "iat": int(time.time()),
    "exp": int(time.time()) + (365 * 24 * 60 * 60), # Valid for 1 year
    "max_amr_fleet": 5                              # Authorized AMR cap
}

# 5. Sign the payload with the private key and drop the license token into the client folder
token = jwt.encode(payload, private_key, algorithm="EdDSA")
with open(os.path.join(DIST_DIR, "license.jwt"), "w") as f:
    f.write(token)

print("\n✨ Generation Complete!")
print(f"📂 Client Assets Folder (SAFE TO SHARE): {DIST_DIR}")
print(f"🔒 Master Signing Key  (KEEP SECRET):   {private_key_path}")