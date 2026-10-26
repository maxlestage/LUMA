# Profils ICC d'essai

Des profils libres (CC0 1.0 Universal, domaine public), lus par `tests/profiles.rs` :

- `CGATS001Compat-v2-micro.icc` : un profil CMJN de presse en version 2, à table lut16
  (CMJN → Lab), 8 Ko ;
- `LAB.icc` : un profil abstrait Lab → Lab en version 4, à table A→B (mAB) et
  description multilingue (mluc).

Les champs d'information de l'en‑tête (module de gestion des couleurs, créateur,
empreinte du profil) ont été mis à zéro ; ils ne servent pas à la lecture.
