//! How the node notifies the owner: Apple Push Notification service and Web Push.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The APNs provider key (token-based authentication) and the app it sends to. The key file stays
/// outside the repository and is never copied into the data directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PushConfig {
    /// The `.p8` file Apple issued for the key.
    pub key_path: PathBuf,
    /// The key's id (10 characters).
    pub key_id: String,
    /// The Apple developer team id (10 characters).
    pub team_id: String,
    /// The app's bundle id, the APNs topic.
    pub bundle_id: String,
    /// Leave every title and label out of a notification (the team's label, a request's or a decision's
    /// title, a device's label), for a node whose data must stay where it is. The notification then
    /// says only what kind it is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_titles: bool,
}

impl WebPushConfig {
    /// Why the node cannot send with this setting: an entry of `extraHosts` that is not a host name (or `*.` and a
    /// domain) in lowercase, such as an address or a form of one (`0x7f.1`).
    pub fn check(&self) -> Result<(), String> {
        if let Some(subject) = &self.subject {
            let is_contact = subject
                .strip_prefix("mailto:")
                .or_else(|| subject.strip_prefix("https://"))
                .is_some_and(|rest| !rest.is_empty() && !rest.contains(char::is_whitespace));
            if !is_contact {
                return Err(format!(
                    "webPush.subject: \"{subject}\" is not a mailto: or https:// address"
                ));
            }
        }
        match self
            .extra_hosts
            .iter()
            .find(|entry| !crate::web_push::is_host_pattern(entry))
        {
            Some(entry) => Err(format!(
                "webPush.extraHosts: \"{entry}\" is not a host name in lowercase, or `*.` and a domain"
            )),
            None => Ok(()),
        }
    }
}

/// Web Push: the node's own VAPID key lives in its data directory, never here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebPushConfig {
    /// The contact a push service may reach the node's operator at: a `mailto:` or `https:` URL. Without
    /// it, the node's own origin (`url`) is used; a personal address is only ever what the owner writes here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// Push services to allow besides the built-in ones (Google, Mozilla, Apple, Microsoft): a host name,
    /// or `*.` and a domain. Never an address.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_hosts: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_web_push_subject_is_a_mailto_or_https_contact() {
        let subject = |text: &str| WebPushConfig {
            subject: Some(text.to_owned()),
            extra_hosts: vec![],
        };
        for good in ["mailto:ops@acme.example", "https://node.acme.example"] {
            assert_eq!(subject(good).check(), Ok(()), "{good}");
        }
        for bad in [
            "",
            "mailto:",
            "ops@acme.example",
            "http://127.0.0.1:4400",
            "https://",
            "mailto:a b@x.example",
        ] {
            let error = subject(bad).check().unwrap_err();
            assert!(error.contains("webPush.subject"), "{bad}: {error}");
        }
    }

    #[test]
    fn web_push_hosts_must_be_names_and_say_which_entry_is_not() {
        let hosts = |list: &[&str]| WebPushConfig {
            subject: None,
            extra_hosts: list.iter().map(|one| (*one).to_owned()).collect(),
        };
        assert_eq!(hosts(&[]).check(), Ok(()));
        assert_eq!(
            hosts(&["push.acme.example", "*.edge.acme.example"]).check(),
            Ok(())
        );
        for bad in [
            "127.0.0.1",
            "0x7f.1",
            "2130706433",
            "*.com",
            "Push.Acme.example",
        ] {
            let error = hosts(&["push.acme.example", bad]).check().unwrap_err();
            assert!(error.contains(bad), "{error}");
        }
    }
}
