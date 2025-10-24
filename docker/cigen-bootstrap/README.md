# CIGen Bootstrap Image

This image packages a ready-to-run copy of `cigen` plus its provider plugins. The CircleCI
setup workflow pulls this image so it can generate the continuation config and compute
skip caches without compiling from source.

## Building

```
# Build multi-arch image (requires buildx)
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --build-arg CIGEN_VERSION=0.1.0 \
  -t docspringcom/cigen:0.1.0 \
  -f docker/cigen-bootstrap/Dockerfile \
  .
```

## Pushing

```
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --build-arg CIGEN_VERSION=0.1.0 \
  -t docspringcom/cigen:0.1.0 \
  -f docker/cigen-bootstrap/Dockerfile \
  --push \
  .
```

Make sure you are logged in (`docker login`) before pushing.

The image installs `cigen` to `/usr/local/bin/cigen` and the plugins to
`/usr/local/lib/cigen/plugins`. The setup workflow points `CIGEN_PLUGIN_DIR` to that
path automatically.
