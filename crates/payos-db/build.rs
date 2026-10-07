// `sqlx::migrate!` yeni eklenen migration dosyalarını tek başına fark etmez.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
