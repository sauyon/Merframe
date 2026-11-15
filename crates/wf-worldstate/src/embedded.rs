#[derive(Debug)]
pub struct SolNode {
    pub value: &'static str,
    pub kind: &'static str,
    pub faction: Option<&'static str>,
    pub levels: Option<(u32, u32)>,
}

#[derive(Debug)]
pub struct SortieBoss {
    pub name: &'static str,
    pub faction: &'static str,
}

#[derive(Debug)]
pub struct SortieModifier {
    pub name: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/tables.rs"));

pub fn lookup<'table, T>(table: &'table [(&str, T)], key: &str) -> Option<&'table T> {
    table
        .binary_search_by_key(&key, |(entry, _)| entry)
        .ok()
        .map(|index| &table[index].1)
}
