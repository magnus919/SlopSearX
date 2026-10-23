//! Qualification markers belong after complete case assertions and cleanup.
pub fn completed(consumer: &str, corpus: &str, id: &str, variant: &str, kind: &str) {
    println!(
        "A06_COMPLETED_SOURCE {}",
        ssx_json::json!({"consumer":consumer,"corpus":corpus,"id":id,"variant":variant,"kind":kind})
    );
}
