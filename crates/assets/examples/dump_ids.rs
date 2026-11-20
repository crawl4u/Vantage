use std::collections::BTreeSet;
fn main() {
    let records = assets::read_registry_for_protocol(
        assets::pinned_block_registry_bytes(),
        assets::active_content_registry_protocol(),
    )
    .unwrap();
    let blocks: BTreeSet<&str> = records.iter().map(|r| r.name.as_ref()).collect();
    let items: BTreeSet<&str> = assets::item::names::identifiers().collect();
    std::fs::write(
        std::env::args().nth(1).unwrap(),
        blocks.into_iter().collect::<Vec<_>>().join("\n"),
    )
    .unwrap();
    std::fs::write(
        std::env::args().nth(2).unwrap(),
        items.into_iter().collect::<Vec<_>>().join("\n"),
    )
    .unwrap();
}
