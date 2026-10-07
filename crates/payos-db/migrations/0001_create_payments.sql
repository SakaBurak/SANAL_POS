-- Durumlar Postgres enum yerine text + CHECK ile tutulur; değer eklemek tek
-- bir constraint değişikliğidir. Domain kurallarının tamamı Payment::restore
-- tarafından yeniden doğrulanır; buradaki kısıtlar son savunma hattıdır.
CREATE TABLE payments (
    id                      uuid        PRIMARY KEY,
    merchant_id             uuid        NOT NULL,
    amount_minor            bigint      NOT NULL CHECK (amount_minor >= 0),
    currency                text        NOT NULL CHECK (currency IN ('TRY', 'USD', 'EUR')),
    status                  text        NOT NULL CHECK (status IN (
                                            'created', 'requires_action', 'authorized',
                                            'captured', 'failed', 'voided',
                                            'partially_refunded', 'refunded',
                                            'unknown', 'disputed'
                                        )),
    refunded_minor          bigint      NOT NULL CHECK (refunded_minor BETWEEN 0 AND amount_minor),
    pending_operation       text        NULL CHECK (pending_operation IN (
                                            'authorize', 'capture', 'void', 'refund'
                                        )),
    pending_refund_minor    bigint      NULL CHECK (pending_refund_minor > 0),
    unknown_previous_status text        NULL CHECK (unknown_previous_status IN (
                                            'created', 'requires_action', 'authorized',
                                            'captured', 'partially_refunded'
                                        )),
    version                 bigint      NOT NULL CHECK (version BETWEEN 1 AND 4294967295),
    created_at              timestamptz NOT NULL,
    updated_at              timestamptz NOT NULL,
    CHECK (updated_at >= created_at),
    CHECK ((status = 'unknown') = (pending_operation IS NOT NULL)),
    CHECK ((pending_operation IS NULL) = (unknown_previous_status IS NULL)),
    CHECK (COALESCE(pending_operation = 'refund', false) = (pending_refund_minor IS NOT NULL))
);

CREATE INDEX payments_merchant_created_idx ON payments (merchant_id, created_at);
