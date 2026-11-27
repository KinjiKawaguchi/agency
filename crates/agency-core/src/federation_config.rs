//! `federation` in `config.json`: how this node takes the signed reads other nodes make of it.
use agency_proto::client::node_id;
use agency_proto::Id;
use serde::{Deserialize, Serialize};

/// The most accounts that may be listed in `unboundReadAccounts`.
pub const MOST_UNBOUND_ACCOUNTS: usize = 100;

/// The section can be left out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct FederationConfig {
    /// Take no signed read that names no node (see "Signed reads" in docs/protocol.md), except the read of the
    /// node's `about`. Turn it on once every node of the org lists `readAudience`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub require_read_audience: bool,
    /// Accounts whose reads that name no node this node still takes, although it has taken a bound read from
    /// them or requires them. For an account that reads from two nodes, one of which is not updated yet.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unbound_read_accounts: Vec<Id>,
}

impl FederationConfig {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Why these settings cannot be used with a node at `url`, in words for the owner.
    pub fn check(&self, url: &str) -> Result<(), String> {
        if self.unbound_read_accounts.len() > MOST_UNBOUND_ACCOUNTS {
            return Err(format!(
                "federation.unboundReadAccounts has more than {MOST_UNBOUND_ACCOUNTS} entries"
            ));
        }
        if self.require_read_audience && node_id(url).is_none() {
            return Err(format!(
                "federation.requireReadAudience needs `url` to be this node's origin written as https://<host>[:<port>] (or http:// on a loopback address); \"{url}\" is not, so other nodes cannot name this node in a read"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_section_can_be_left_out_and_is_then_written_as_nothing() {
        let config: FederationConfig = serde_json::from_str("{}").unwrap();
        assert!(config.is_default());
        assert_eq!(serde_json::to_string(&config).unwrap(), "{}");
    }

    #[test]
    fn an_account_that_is_no_id_is_refused_when_the_section_is_read() {
        let good: FederationConfig =
            serde_json::from_str(r#"{"unboundReadAccounts":["alice","bob-2"]}"#).unwrap();
        assert_eq!(good.unbound_read_accounts.len(), 2);
        for bad in [r#"["Alice"]"#, r#"["a b"]"#, r#"[""]"#, r#"[1]"#] {
            let text = format!(r#"{{"unboundReadAccounts":{bad}}}"#);
            assert!(
                serde_json::from_str::<FederationConfig>(&text).is_err(),
                "{bad}"
            );
        }
        assert!(serde_json::from_str::<FederationConfig>(r#"{"unknown":1}"#).is_err());
    }

    #[test]
    fn what_cannot_be_used_is_said_in_words_for_the_owner() {
        let many = FederationConfig {
            unbound_read_accounts: (0..=MOST_UNBOUND_ACCOUNTS)
                .map(|n| Id::parse(&format!("a{n}")).unwrap())
                .collect(),
            ..FederationConfig::default()
        };
        assert!(many
            .check("https://work.example.com")
            .unwrap_err()
            .contains("100"));
        let required = FederationConfig {
            require_read_audience: true,
            ..FederationConfig::default()
        };
        assert_eq!(required.check("https://work.example.com"), Ok(()));
        let error = required.check("http://work.example.com:4400").unwrap_err();
        assert!(error.contains("federation.requireReadAudience"), "{error}");
        assert_eq!(
            FederationConfig::default().check("http://lan.example:4400"),
            Ok(())
        );
    }
}
