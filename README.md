

## Generating the SDK 

```bash
openapi-generator-cli generate \
    -i openapi.json \
    -g rust \
    -o rust-client-sdk \
    --additional-properties=packageName=my_sdk \
    --skip-validate-spec
```
