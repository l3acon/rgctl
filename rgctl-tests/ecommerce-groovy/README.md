# ecommerce-groovy

Minimal Groovy slice for dashboard / GQL / CFG smoke (same role as `ecommerce-ruby`).

```bash
cargo build --release --bin rgctl
cd rgctl-tests/ecommerce-groovy && ../../target/release/rgctl discover . -l groovy --with-cfg --with-taint
```

Optional: `./rgctl-tests/gql-verification-smoke/verify-extraction-gql-groovy.sh`
