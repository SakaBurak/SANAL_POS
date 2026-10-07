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

## Local Development

`payos-db` testleri canlı bir PostgreSQL ister. `#[sqlx::test]` her test için
ayrı bir veritabanı açıp migration'ları uygular ve test bitince siler.

```powershell
docker compose -f infra/docker-compose.yml up -d --wait
$env:DATABASE_URL = "postgres://payos:payos@localhost:55432/payos"   # bash: export DATABASE_URL=...
cargo test --workspace
```

`DATABASE_URL` için `.env.example` dosyasını `.env` olarak kopyalamak da
yeterlidir. Docker'daki Postgres, makinede kurulu bir PostgreSQL ile
çakışmaması için 55432 portunu kullanır.

## Security

API anahtarları, banka bilgileri, kart verileri, private key'ler ve production secret'ları repository'e commit edilmemelidir.

## License

Proprietary.