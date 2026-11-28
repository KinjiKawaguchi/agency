//! Cloudflare Access in front of the Client API.
use serde::{Deserialize, Serialize};

/// Cloudflare Access for requests that arrive through a tunnel or at the public host name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessConfig {
    /// The public host name the tunnel serves, such as `agency.example.com`.
    pub public_host: String,
    /// The Zero Trust team domain, such as `acme.cloudflareaccess.com`.
    pub team_domain: String,
    /// The Access application's audience tag (64 hex characters).
    pub audience: String,
    /// Only this signed-in email may write: writes act for the node's account (they relay its
    /// decisions and device changes).
    pub owner_email: String,
    /// The iOS app that may receive the Access session after a sign-in outside its web view,
    /// `<team id>.<bundle id>`. Without it (or with a malformed one) the node has no handoff route and
    /// no app association file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// Whether the app may sign in through its custom scheme (`via=scheme`), for a host that is not in the app
    /// build's associated domains. Off by default: the scheme is delivered to the app that starts the sign-in,
    /// whatever app that is, so an app of someone else's on the owner's device could start it and redeem the
    /// owner's token; the universal link of `via=link` reaches only the app the association file names.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_scheme_sign_in: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_apps_scheme_sign_in_is_off_unless_the_access_setting_turns_it_on() {
        let base = r#""publicHost":"agency.example.com","teamDomain":"acme.cloudflareaccess.com","audience":"a","ownerEmail":"alice@example.com""#;
        let without: AccessConfig = serde_json::from_str(&format!("{{{base}}}")).unwrap();
        assert!(
            !without.allow_scheme_sign_in,
            "a configuration from before keeps the scheme off"
        );
        assert!(!serde_json::to_string(&without)
            .unwrap()
            .contains("allowSchemeSignIn"));
        let with: AccessConfig =
            serde_json::from_str(&format!("{{{base},\"allowSchemeSignIn\":true}}")).unwrap();
        assert!(with.allow_scheme_sign_in);
        assert!(serde_json::to_string(&with)
            .unwrap()
            .contains("\"allowSchemeSignIn\":true"));
    }
}
