#!/usr/bin/env python3
"""Les échanges avec l'API App Store Connect, pour publier LÜMA sans Mac à soi : la CI
macOS crée l'identifiant de l'app, un certificat de distribution et un profil App Store,
puis les retire après l'envoi.

La clé API vient de l'environnement : ASC_KEY_ID, ASC_ISSUER_ID et ASC_KEY (le contenu
du fichier .p8). Dépendances : pyjwt et cryptography (pip).

    asc.py bundle-id app.luma.studio LUMA          → id de l'identifiant (créé au besoin)
    asc.py app app.luma.studio                     → id de la fiche App Store (sinon erreur)
    asc.py certificate demande.csr sortie.cer      → id du certificat créé
    asc.py profile <bundle-id> <cert-id> sortie    → id du profil créé
    asc.py delete certificates|profiles <id>       → retire un certificat ou un profil
"""

import base64
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

import jwt

API = "https://api.appstoreconnect.apple.com/v1"


def secret(name):
    """Un secret tel qu'on l'a collé : sans espaces ni retours à la ligne autour."""
    return os.environ[name].replace("\r", "").strip()


def token():
    """Un jeton signé ES256, valable 20 minutes."""
    now = int(time.time())
    return jwt.encode(
        {"iss": secret("ASC_ISSUER_ID"), "iat": now, "exp": now + 1200, "aud": "appstoreconnect-v1"},
        secret("ASC_KEY") + "\n",
        algorithm="ES256",
        headers={"kid": secret("ASC_KEY_ID"), "typ": "JWT"},
    )


def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(
        API + path,
        data=data,
        method=method,
        headers={"Authorization": "Bearer " + token(), "Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req) as r:
            text = r.read()
            return json.loads(text) if text else {}
    except urllib.error.HTTPError as e:
        detail = e.read().decode(errors="replace")
        sys.exit(f"App Store Connect : {method} {path} → {e.code}\n{detail}")


def bundle_id(identifier, name):
    query = urllib.parse.urlencode({"filter[identifier]": identifier})
    found = [i["id"] for i in call("GET", f"/bundleIds?{query}")["data"] if i["attributes"]["identifier"] == identifier]
    if found:
        bundle = found[0]
    else:
        created = call(
            "POST",
            "/bundleIds",
            {"data": {"type": "bundleIds", "attributes": {"identifier": identifier, "name": name, "platform": "IOS"}}},
        )
        bundle = created["data"]["id"]
    # « Se connecter avec Apple » (l'accès offert) : le profil en porte alors le droit.
    sign_in_with_apple(bundle)
    return bundle


def sign_in_with_apple(bundle):
    capabilities = call("GET", f"/bundleIds/{bundle}/bundleIdCapabilities")["data"]
    if any(c["attributes"]["capabilityType"] == "APPLE_ID_AUTH" for c in capabilities):
        return
    call(
        "POST",
        "/bundleIdCapabilities",
        {
            "data": {
                "type": "bundleIdCapabilities",
                "attributes": {
                    "capabilityType": "APPLE_ID_AUTH",
                    "settings": [{"key": "APPLE_ID_AUTH_APP_CONSENT", "options": [{"key": "PRIMARY_APP_CONSENT"}]}],
                },
                "relationships": {"bundleId": {"data": {"type": "bundleIds", "id": bundle}}},
            }
        },
    )


def app(identifier):
    query = urllib.parse.urlencode({"filter[bundleId]": identifier})
    found = call("GET", f"/apps?{query}")["data"]
    if not found:
        sys.exit(
            f"Pas encore de fiche App Store pour {identifier} : dans App Store Connect, "
            "Apps › + › Nouvelle app (iOS, identifiant de lot "
            f"{identifier}), puis relancer."
        )
    return found[0]["id"]


def certificate(csr_path, out):
    csr = open(csr_path).read()
    created = call(
        "POST",
        "/certificates",
        {"data": {"type": "certificates", "attributes": {"csrContent": csr, "certificateType": "DISTRIBUTION"}}},
    )
    data = created["data"]
    with open(out, "wb") as f:
        f.write(base64.b64decode(data["attributes"]["certificateContent"]))
    return data["id"]


def profile(bundle, cert, out):
    created = call(
        "POST",
        "/profiles",
        {
            "data": {
                "type": "profiles",
                "attributes": {"name": f"LUMA App Store {int(time.time())}", "profileType": "IOS_APP_STORE"},
                "relationships": {
                    "bundleId": {"data": {"type": "bundleIds", "id": bundle}},
                    "certificates": {"data": [{"type": "certificates", "id": cert}]},
                },
            }
        },
    )
    data = created["data"]
    with open(out, "wb") as f:
        f.write(base64.b64decode(data["attributes"]["profileContent"]))
    return data["id"]


def main(args):
    match args:
        case ["bundle-id", identifier, name]:
            print(bundle_id(identifier, name))
        case ["app", identifier]:
            print(app(identifier))
        case ["certificate", csr, out]:
            print(certificate(csr, out))
        case ["profile", bundle, cert, out]:
            print(profile(bundle, cert, out))
        case ["delete", kind, ident] if kind in ("certificates", "profiles"):
            call("DELETE", f"/{kind}/{ident}")
        case _:
            sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
