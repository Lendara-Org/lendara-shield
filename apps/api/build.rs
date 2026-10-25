// Recompile when migrations change: `sqlx::migrate!` embeds them at build time.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
