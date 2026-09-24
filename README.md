

## Generating the SDK 

```bash
make generate
```

### Reverse proxy trust

By default rate limits use the connection's peer IP and ignore forwarded headers.
Set `ENVX_TRUST_PROXY=true` only when all incoming requests pass through a trusted
proxy that overwrites `X-Real-IP`, and untrusted callers cannot connect directly.
This uses a single valid `X-Real-IP`; `X-Forwarded-For` is never trusted. Railway's
HTTP ingress provides `X-Real-IP` (see its public networking specs). Self-hosted
operators must configure their ingress accordingly before enabling this option.
