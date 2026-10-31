#!/usr/bin/env python3
"""Minimal App Store Connect API client for iOS signing in CI (stdlib + the openssl CLI).

Reads ASC_KEY_ID, ASC_ISSUER_ID and ASC_KEY_PATH (the .p8 file) from the environment.
Each TestFlight run makes a throwaway distribution certificate and profile and removes them
at the end; builds already uploaded are not affected by that.

  scripts/asc.py create-cert <csr.pem> <out-cert.pem>      prints the certificate id
  scripts/asc.py create-profile <bundle-id> <cert-id> <out.mobileprovision>
                                                           prints "<profile id> <UUID>"
  scripts/asc.py revoke-cert <cert-id>
  scripts/asc.py delete-profile <profile-id>
"""
import base64
import json
import os
import plistlib
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

API = "https://api.appstoreconnect.apple.com/v1"


def b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def der_to_raw(der: bytes) -> bytes:
    """ECDSA signature from DER (SEQUENCE of two INTEGERs) to JWS's raw r || s."""
    if der[0] != 0x30:
        raise ValueError("not a DER sequence")
    i = 2 if der[1] < 0x80 else 2 + (der[1] & 0x7F)
    raw = b""
    for _ in range(2):
        if der[i] != 0x02:
            raise ValueError("not a DER integer")
        n = der[i + 1]
        raw += der[i + 2 : i + 2 + n].lstrip(b"\0").rjust(32, b"\0")
        i += 2 + n
    return raw


def token() -> str:
    now = int(time.time())
    header = b64url(json.dumps({"alg": "ES256", "kid": os.environ["ASC_KEY_ID"], "typ": "JWT"}).encode())
    claims = b64url(json.dumps({"iss": os.environ["ASC_ISSUER_ID"], "iat": now, "exp": now + 600,
                                "aud": "appstoreconnect-v1"}).encode())
    signing_input = f"{header}.{claims}".encode()
    der = subprocess.run(["openssl", "dgst", "-sha256", "-sign", os.environ["ASC_KEY_PATH"]],
                         input=signing_input, capture_output=True, check=True).stdout
    return f"{header}.{claims}.{b64url(der_to_raw(der))}"


def call(method: str, path: str, body: dict | None = None) -> dict:
    req = urllib.request.Request(
        path if path.startswith("https://") else API + path, method=method,
        data=json.dumps(body).encode() if body else None,
        headers={"Authorization": f"Bearer {token()}", "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req) as res:
            text = res.read()
            return json.loads(text) if text else {}
    except urllib.error.HTTPError as e:
        sys.exit(f"{method} {path}: HTTP {e.code} {e.read().decode()}")


def pem_body(pem: str) -> str:
    return "".join(re.findall(r"^[A-Za-z0-9+/=]+$", pem, re.M))


def create_cert(csr_path: str, out: str) -> None:
    csr = pem_body(open(csr_path).read())
    data = call("POST", "/certificates", {"data": {"type": "certificates", "attributes": {
        "csrContent": csr, "certificateType": "DISTRIBUTION"}}})["data"]
    content = data["attributes"]["certificateContent"]
    lines = [content[i : i + 64] for i in range(0, len(content), 64)]
    with open(out, "w") as f:
        print("-----BEGIN CERTIFICATE-----", *lines, "-----END CERTIFICATE-----", sep="\n", file=f)
    print(data["id"])


def create_profile(bundle: str, cert_id: str, out: str) -> None:
    ids = call("GET", f"/bundleIds?filter[identifier]={bundle}")["data"]
    bundle_id = next((b["id"] for b in ids if b["attributes"]["identifier"] == bundle), None)
    if bundle_id is None:
        sys.exit(f"bundle id {bundle} is not registered")
    data = call("POST", "/profiles", {"data": {
        "type": "profiles",
        "attributes": {"name": f"DiBillSion CI {int(time.time())}", "profileType": "IOS_APP_STORE"},
        "relationships": {
            "bundleId": {"data": {"type": "bundleIds", "id": bundle_id}},
            "certificates": {"data": [{"type": "certificates", "id": cert_id}]},
        }}})["data"]
    raw = base64.b64decode(data["attributes"]["profileContent"])
    with open(out, "wb") as f:
        f.write(raw)
    # The plist sits inside a CMS envelope; its UUID names the profile for Xcode.
    start, end = raw.index(b"<?xml"), raw.index(b"</plist>") + len(b"</plist>")
    print(data["id"], plistlib.loads(raw[start:end])["UUID"])


if __name__ == "__main__":
    match sys.argv[1:]:
        case ["create-cert", csr, out]:
            create_cert(csr, out)
        case ["create-profile", bundle, cert_id, out]:
            create_profile(bundle, cert_id, out)
        case ["revoke-cert", cert_id]:
            call("DELETE", f"/certificates/{cert_id}")
        case ["delete-profile", profile_id]:
            call("DELETE", f"/profiles/{profile_id}")
        case _:
            sys.exit(__doc__)
