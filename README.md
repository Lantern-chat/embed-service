Lantern Embed Service
=====================

Originally part of Lantern's main server architecture, the data structures and microservice for handling website embeds have been moved here.

The purpose of these is to parse websites and generate previews for use in other websites.

## Docker Usage

```
docker run -v /path/to/my/config.prod.toml:/config/config.toml \
  -e EMBED_BIND_ADDRESS="0.0.0.0:8050" \
  ghcr.io/lantern-chat/embed-service:latest
```

Without specifying the config volume, it'll default to the config file from `docker/config.default.toml`.