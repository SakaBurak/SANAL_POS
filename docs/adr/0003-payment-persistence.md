# ADR 0003: Ödeme kalıcılığı (PostgreSQL)

- Durum: Kabul edildi
- Tarih: 2026-10-07
- Kapsam: `crates/payos-db`, `crates/payos-domain` (snapshot/restore)

## Bağlam

`Payment` aggregate'i (ADR 0002) PostgreSQL'e yazılıp geri okunduğunda bire
bir aynı olmalı. Ayrıca aynı ödemeyi eşzamanlı değiştiren iki istek
birbirinin yazdığını sessizce ezmemeli. API katmanı henüz yok; bu karar
yalnızca Payment, PostgreSQL ve tekrar Payment dönüşümünü kapsar.

## Karar

### Şema: `text` + `CHECK`, `bigint` version

- Durum, para birimi ve bekleyen işlem Postgres `enum` tipi yerine `text`
  sütunlarda tutulur ve `CHECK` ile sınırlandırılır. Yeni değer eklemek tek
  bir constraint değişikliğidir; `ALTER TYPE ... ADD VALUE` kısıtları yoktur.
- Tutarlar `bigint` minor unit, para birimi ISO 4217 kodudur (ADR 0001).
- `version` sütunu `bigint`'tir. Domain'deki `u32` kayıpsız sığar;
  `CHECK (version BETWEEN 1 AND 4294967295)` ile sınırlıdır ve okurken
  `u32::try_from` ile dönüştürülür.
- `Unknown` bağlamı üç sütundur: `pending_operation`,
  `pending_refund_minor`, `unknown_previous_status`. Birlikte var ya da
  birlikte yok olmaları ve yalnızca `status = 'unknown'` iken dolu olmaları
  `CHECK` ile zorlanır. Bu kodlama bir persistence detayı olduğu için
  domain'de değil `payos-db` içindedir.
- `CHECK` kısıtları son savunma hattıdır; tüm domain kurallarını tekrar etmez.

### Restore domain kurallarını yeniden doğrular

`Payment::restore(PaymentSnapshot)` kayıttan gelen veriyi komutlarla
ulaşılabilecek bir duruma karşı doğrular: iade tutarının durumla ve para
birimiyle tutarlılığı, `Unknown` bağlamının bekleyen işlemle uyumu,
`version >= 1` ve `updated_at >= created_at`. Kuralları geçemeyen satır
sessizce yüklenmez; repository `CorruptRow` döner. Böylece elle yapılan bir
SQL düzeltmesi veya hatalı bir migration bozuk bir aggregate üretemez.

### Optimistic locking

`update(conn, &payment, expected_version)` yalnızca değişebilen sütunları
(`status`, iade, bekleyen işlem, `version`, `updated_at`) yazar:

```sql
UPDATE payments SET ... WHERE id = $1 AND version = $2
```

- Etkilenen satır yoksa `SELECT version` ile ayrım yapılır: satır yoksa
  `NotFound`, varsa `VersionConflict { expected, actual }`. Çağıran taraf
  çakışmada ödemeyi yeniden okuyup komutu tekrar değerlendirir.
- `payment.version() <= expected_version` ise yazılacak değişiklik yoktur;
  DB'ye gidilmeden `NothingToUpdate` döner.
- `merchant_id`, tutar, para birimi ve `created_at` hiç güncellenmez.
- Metotlar `&mut PgConnection` alır; havuz bağlantısıyla da transaction
  içinde de çalışır. İleride ödeme, defter ve outbox aynı transaction'da
  yazılabilir.

### Zaman çözünürlüğü: mikrosaniye

PostgreSQL `timestamptz` mikrosaniye, `OffsetDateTime` nanosaniye tutar.
Domain, `Payment::new` ve her komutta zamanı UTC'ye çevirip mikrosaniyeye
keser; `updated_at` monotonluk kontrolü kesilmiş değerle yapılır. Böylece
yazılıp okunan nesne eşitliği korunur. Mikrosaniyeden ince sıralama ödeme
akışı için anlamlı değildir.

### Çalışma zamanı sorguları

Sorgular `sqlx::query` ile çalışma zamanında hazırlanır, satırlar elle
eşlenir. Derleme zamanında doğrulanan `query!` makroları derleme sırasında
canlı DB veya `.sqlx` önbelleği ister; şema oturana kadar ertelendi.
Repository testleri her sorguyu gerçek PostgreSQL'e karşı çalıştırır.

### Migration ve testler

- Migration'lar `sqlx::migrate!()` ile ikiliye gömülür (`payos_db::MIGRATOR`).
  `build.rs`, yeni migration dosyası eklendiğinde yeniden derlemeyi tetikler.
- Testler `#[sqlx::test(migrator = "payos_db::MIGRATOR")]` ile her test için
  ayrı, boş bir veritabanında çalışır. Yerelde `infra/docker-compose.yml`
  (host portu 55432), CI'da `postgres:18` service kullanılır.

## Kapsam dışı ve gelecek notları

- Partitioning ertelendi. `created_at` ve UUIDv7 `id`, ileride zamana göre
  partition'lamaya izin verecek biçimde tutuluyor; `(merchant_id, created_at)`
  index'i merchant bazlı listelemeye hazırlıktır.
- Derleme zamanında doğrulanan sorgular, TLS, bağlantı havuzu ayarları.
- Outbox ve ledger, API katmanı, ortak test yardımcıları (`payos-testkit`).
