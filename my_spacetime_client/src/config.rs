/**
 * SpacetimeDB Client Configuration
 * 
 */

/// URI for connecting to the SpacetimeDB server
pub const SERVER_URI: &str = "http://localhost:3000";

/// Name of the SpacetimeDB module/database to connect to
pub const MODULE_NAME: &str = "mydatabase";

/// Retrieve authentication token from the environment
pub fn get_token() -> Option<String> {
    std::env::var("SPACETIME_TOKEN").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_uri_valid() {
        assert!(SERVER_URI.starts_with("http"));
    }

    #[test]
    fn module_name_no_underscores() {
        assert!(!MODULE_NAME.contains('_'));
    }
}
