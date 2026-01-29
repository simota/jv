# Releasing

Releases are built by the GitHub Actions workflow `/.github/workflows/release.yml`
when a GitHub Release is published.

## Key constraints

- `CGO_ENABLED=0` is required to avoid glibc version dependencies on Linux.

## Steps

1. Ensure tests pass:
   - `go test ./...`
2. Create and push a tag (e.g. `v0.1.1`).
3. Publish a GitHub Release for that tag (the workflow triggers on release publish).

## Local build (optional dry run)

Example for Linux amd64 (artifact name matches the release workflow):

```bash
VERSION=v0.1.1
mkdir -p dist
GOOS=linux GOARCH=amd64 CGO_ENABLED=0 \
  go build -o "dist/jv_${VERSION}_${GOOS}_${GOARCH}" ./cmd/jv

tar -czf "dist/jv_${VERSION}_${GOOS}_${GOARCH}.tar.gz" -C dist "jv_${VERSION}_${GOOS}_${GOARCH}"
```
