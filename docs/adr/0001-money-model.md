# ADR 0001: Para modeli (Money, MinorAmount, Currency)

- Durum: Kabul edildi
- Tarih: 2026-10-07
- Kapsam: `crates/payos-core/src/money.rs`, `crates/payos-core/src/error.rs`

## Bağlam

Ödeme sisteminde para hesabındaki küçük bir hata (yuvarlama, taşma, farklı para
birimlerinin toplanması) doğrudan para kaybı ve defter tutarsızlığı demektir.
Mimari doküman bu nedenle şu ilkeleri koyar: para kayan nokta ile tutulmaz,
para birimi tutarla birlikte taşınır ve aritmetik sessizce hata yapamaz.

`payos-core` ağ, disk ve zaman bağımlılığı olmayan saf domain mantığı taşır;
aynı kod ileride WASM, mobil ve masaüstü hedeflerine de derlenecektir.

## Karar

- Tutar, para biriminin en küçük birimi cinsinden `i64` olarak tutulur
  (`MinorAmount`; ör. `1000` = 10,00 TRY). Kayan nokta kullanılmaz. `i64`,
  PostgreSQL `BIGINT` ile birebir eşleşir.
- `Money` her zaman tutarı ve para birimini birlikte taşır. Alanlar private'tır;
  değerler yalnızca `Money::new` ve `Money::zero` ile oluşturulur.
- `Money` ve `MinorAmount` negatif olamaz; negatif girdi `NegativeAmount` döner.
- Aritmetik yalnızca `checked_add` ve `checked_sub` ile yapılır:
  - Farklı para birimi: `CurrencyMismatch`
  - Taşma: `Overflow`
  - Çıkarmada sonuç negatife düşecekse: `InsufficientAmount`
- `Add`, `Sub`, `Mul` operatörleri ve `PartialOrd` bilerek uygulanmaz. Farklı
  para birimlerindeki tutarlar ne toplanabilir ne de karşılaştırılabilir.
- `Currency` şimdilik `TRY`, `USD`, `EUR` ile sınırlıdır. `FromStr` yalnızca
  büyük harfli ISO 4217 koduyla tam eşleşmeyi kabul eder; `"TL"`, `"try"`,
  `" TRY"` gibi girdiler `InvalidCurrency` döner.
- `Display` yerelleştirme yapmaz (`"1250 TRY"`). `12,50 TL` gibi biçimlendirme
  UI ve SDK katmanının işidir.
- Hata tipi `MoneyError`, `thiserror` ile tanımlanır.

## Sonuçlar

- Para birimi karışması ve taşma derleme zamanında değil ama her zaman açık bir
  `Result` hatası olarak yakalanır; sessiz yanlış sonuç üretilemez.
- Çağıran kod her aritmetikte hatayı ele almak zorundadır; bu bilinçli bir
  sürtünmedir.
- Her para birimi için ondalık hane sayısı `Currency::minor_unit_digits` ile
  açıkça tanımlıdır (şu an üçü için de 2).

## Gelecek notları

Aşağıdakiler şimdilik uygulanmayacaktır; ihtiyaç doğduğunda bu ADR
güncellenecek veya yeni bir ADR yazılacaktır.

1. **Karşılaştırma:** Tutar karşılaştırması gerekirse para birimi kontrolü yapan
   açık bir metot eklenecek, ör.
   `checked_cmp(self, rhs) -> Result<Ordering, MoneyError>`. `PartialOrd` yine
   eklenmeyecek.
2. **Negatif para ve defter yönü:** İade ve defter kayıtlarında "negatif para"
   ihtiyacı `Money` içine sokulmayacak. Debit/credit yönü ledger tarafında ayrı
   bir modelle tutulacak (ör. `Direction` ile `Money` çifti). `Money` için
   negatif yasağı korunacak.
3. **Para birimi listesinin büyümesi:** ISO 4217 desteği genişlerse enum'u
   şişirmek yerine ayrı bir currency registry yaklaşımı (kod, ondalık hane
   sayısı, aktiflik bilgisi) değerlendirilecek.
