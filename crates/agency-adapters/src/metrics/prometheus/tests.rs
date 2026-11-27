use super::*;
use agency_proto::metrics::{
    SeriesKind::{Call, Serve},
    BUCKETS,
};

fn counts(spec: &[(usize, u64)], errors: u64, sum_ms: u64) -> Counts {
    let mut buckets = [0; BUCKETS];
    for (index, count) in spec {
        buckets[*index] = *count;
    }
    Counts {
        count: buckets.iter().sum(),
        errors,
        sum_ms,
        buckets,
    }
}

#[test]
fn a_series_is_a_cumulative_histogram_with_its_sum_count_and_errors() {
    let totals = [(
        SeriesKey::new(Serve, "GET /api/me", "2xx"),
        counts(&[(0, 5), (4, 4), (9, 1)], 0, 985),
    )];
    let text = text(&totals);
    let line = |needle: &str| {
        text.lines()
            .find(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("{needle}: {text}"))
    };
    let labels = r#"kind="serve",name="GET /api/me",status="2xx""#;
    assert_eq!(
        line(r#"le="1""#),
        format!("agency_duration_milliseconds_bucket{{{labels},le=\"1\"}} 5")
    );
    assert_eq!(
        line(r#"le="10""#),
        format!("agency_duration_milliseconds_bucket{{{labels},le=\"10\"}} 5"),
        "cumulative"
    );
    assert_eq!(
        line(r#"le="25""#),
        format!("agency_duration_milliseconds_bucket{{{labels},le=\"25\"}} 9")
    );
    assert_eq!(
        line(r#"le="1000""#),
        format!("agency_duration_milliseconds_bucket{{{labels},le=\"1000\"}} 10")
    );
    assert_eq!(
        line(r#"le="30000""#),
        format!("agency_duration_milliseconds_bucket{{{labels},le=\"30000\"}} 10")
    );
    assert_eq!(
        line(r#"le="+Inf""#),
        format!("agency_duration_milliseconds_bucket{{{labels},le=\"+Inf\"}} 10")
    );
    assert_eq!(
        line("_sum"),
        format!("agency_duration_milliseconds_sum{{{labels}}} 985")
    );
    assert_eq!(
        line("_count"),
        format!("agency_duration_milliseconds_count{{{labels}}} 10")
    );
    assert_eq!(
        line("agency_errors_total{"),
        format!("agency_errors_total{{{labels}}} 0")
    );
    assert!(text.contains("# TYPE agency_duration_milliseconds histogram\n"));
    assert!(text.contains("# TYPE agency_errors_total counter\n"));
    assert_eq!(
        text.lines().filter(|l| l.contains("_bucket")).count(),
        15,
        "14 bounds and +Inf"
    );
}

#[test]
fn the_labels_hold_the_peer_and_client_where_a_series_has_them_and_escape_what_needs_it() {
    let totals = [
        (
            SeriesKey::new(Call, "peer get_view", "ok").with_peer("b.example"),
            counts(&[(3, 1)], 0, 8),
        ),
        (
            SeriesKey::new(Serve, "GET /a\"b\\c", "2xx"),
            counts(&[(3, 1)], 0, 8),
        ),
        (
            SeriesKey::new(SeriesKind::Client, "GET /x", "network").with_client("ios"),
            counts(&[(3, 1)], 1, 8),
        ),
    ];
    let text = text(&totals);
    assert!(
        text.contains(r#"kind="call",name="peer get_view",status="ok",peer="b.example",le="1""#),
        "{text}"
    );
    assert!(text.contains(r#"name="GET /a\"b\\c""#), "{text}");
    assert!(
        text.contains(r#"kind="client",name="GET /x",status="network",client="ios"} 1"#),
        "{text}"
    );
    assert!(
        !text.contains("peer=\"\""),
        "a series without a peer has no peer label"
    );
}

#[test]
fn nothing_recorded_is_only_the_headers() {
    let text = text(&[]);
    assert_eq!(text.lines().count(), 4);
    assert!(text.lines().all(|l| l.starts_with('#')));
}

#[test]
fn a_cumulative_bucket_stops_at_what_a_count_holds_and_never_wraps_around() {
    let mut buckets = [0; BUCKETS];
    buckets[0] = u64::MAX;
    buckets[1] = 5;
    buckets[2] = 7;
    let totals = [(
        SeriesKey::new(Serve, "GET /api/me", "2xx"),
        Counts {
            count: u64::MAX,
            errors: 0,
            sum_ms: 0,
            buckets,
        },
    )];
    let text = text(&totals);
    let largest = u64::MAX.to_string();
    let second = text
        .lines()
        .filter(|line| line.starts_with("agency_duration_milliseconds_bucket"))
        .nth(2)
        .unwrap();
    assert!(
        second.ends_with(&format!("}} {largest}")),
        "a wrapped sum would have been 4: {second}"
    );
}
