pub mod error;
pub mod data;
pub mod mod_db;
pub mod build;
pub mod passive_tree;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver() {
        let v = super::version();
        assert!(v.contains('.'));
    }
}
