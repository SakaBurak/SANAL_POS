# SANAL_POS

Sanal POS ve ödeme orkestrasyonu platformu.

## Project Status

Early development / architecture phase.

## Backend

- Rust
- PostgreSQL
- SQLx
- Axum

## Repository Structure

- `crates/payos-core` - Temel tipler ve ortak iş kuralları
- `crates/payos-domain` - Ödeme domain modeli ve durum makinesi
- `crates/payos-db` - PostgreSQL erişimi ve migration'lar
- `crates/payos-api` - HTTP API
- `docs` - Teknik mimari ve ADR dokümanları
- `infra` - Altyapı dosyaları

## Security

API anahtarları, banka bilgileri, kart verileri, private key'ler ve production secret'ları repository'e commit edilmemelidir.

## License

Proprietary.