

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

Railway deployment checklist:

- Route public requests through Railway HTTP ingress and prevent untrusted direct access.
- Set `ENVX_TRUST_PROXY=true` on the API service before deploying.
- Verify the ingress supplies one valid `X-Real-IP`; missing or malformed values fall back to the peer IP.
- Keep this flag unset for direct connections or proxies that do not overwrite client-supplied `X-Real-IP`.

Railway documents its client IP header in the [public networking specifications](https://docs.railway.com/networking/public-networking/specs-and-limits).
