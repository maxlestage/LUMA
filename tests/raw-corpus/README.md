# Corpus de RAW de test

Les fichiers ne sont pas versionnés (plusieurs dizaines de Mo chacun). Pour les
télécharger depuis [raw.pixls.us](https://raw.pixls.us/) :

```sh
scripts/fetch-corpus.sh
```

`cargo test` développe alors chacun d'eux (`crates/luma-render/tests/corpus.rs`) ; sans
corpus, ce test est sauté. Pour ajouter un boîtier, ajouter son chemin dans le script.
