//! The Google Calendar API calls the adapter makes: the calendar list,
//! `events.list` (first read and by sync token), and the writes (insert,
//! patch, delete, instances).
//!
//! Errors carry what a run decides on, never a body: a 410 (the sync token
//! is gone), the calendar API not being enabled for the project, a failed
//! `If-Match` (412) and an id that exists already (409).

use std::collections::HashMap;

use serde::Deserialize;
use url::Url;

use super::{ProviderError, classify, transport};

/// Why a Calendar call did not give an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarError {
    /// Trouble every provider call can meet.
    Provider(ProviderError),
    /// 410: the sync token is no longer accepted; read in full.
    Gone,
    /// 403 `accessNotConfigured`: the Calendar API is not enabled for the
    /// Google project this instance is registered as.
    ApiNotEnabled,
    /// 412: the event's etag is not the one sent in `If-Match`.
    PreconditionFailed,
    /// 409: an event with that id exists already.
    Exists,
}

impl From<ProviderError> for CalendarError {
    fn from(error: ProviderError) -> Self {
        Self::Provider(error)
    }
}

/// The Calendar API for one access token.
#[derive(Debug)]
pub struct Calendar<'a> {
    http: &'a reqwest::Client,
    base: &'a Url,
    token: &'a str,
}

/// One calendar of `calendarList.list`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEntry {
    /// Its id (an address for most).
    pub id: String,
    /// Its name.
    #[serde(default)]
    pub summary: Option<String>,
    /// The member's own name for it.
    #[serde(default)]
    pub summary_override: Option<String>,
    /// Its colour.
    #[serde(default)]
    pub background_color: Option<String>,
    /// Its IANA time zone.
    #[serde(default)]
    pub time_zone: Option<String>,
    /// The account's primary calendar.
    #[serde(default)]
    pub primary: bool,
    /// `owner`, `writer`, `reader` or `freeBusyReader`.
    #[serde(default)]
    pub access_role: Option<String>,
    /// Deleted from the list.
    #[serde(default)]
    pub deleted: bool,
}

impl CalendarEntry {
    /// Whether events may be written on it.
    pub fn writable(&self) -> bool {
        matches!(self.access_role.as_deref(), Some("owner" | "writer"))
    }

    /// What to call it: the member's name for it, else Google's.
    pub fn name(&self) -> String {
        self.summary_override
            .as_deref()
            .or(self.summary.as_deref())
            .unwrap_or(&self.id)
            .to_owned()
    }
}

/// When an event starts or ends: a date, or a time and its zone.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventTime {
    /// `YYYY-MM-DD` (all-day).
    #[serde(default)]
    pub date: Option<String>,
    /// RFC 3339 with an offset.
    #[serde(default)]
    pub date_time: Option<String>,
    /// The IANA zone it was written in.
    #[serde(default)]
    pub time_zone: Option<String>,
}

/// An organiser or attendee.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    /// Their address.
    #[serde(default)]
    pub email: Option<String>,
    /// Their name.
    #[serde(default)]
    pub display_name: Option<String>,
    /// `needsAction`, `accepted`, `tentative` or `declined`.
    #[serde(default)]
    pub response_status: Option<String>,
    /// The member whose calendar it is.
    #[serde(default, rename = "self")]
    pub is_self: bool,
}

/// `extendedProperties`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExtendedProperties {
    /// What an app keeps on the event for itself.
    #[serde(default)]
    pub private: HashMap<String, String>,
}

/// One event of `events.list` or `events.get`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleEvent {
    /// Its id.
    pub id: String,
    /// Its etag.
    #[serde(default)]
    pub etag: Option<String>,
    /// `confirmed`, `tentative` or `cancelled`.
    #[serde(default)]
    pub status: Option<String>,
    /// Its title.
    #[serde(default)]
    pub summary: Option<String>,
    /// Its place.
    #[serde(default)]
    pub location: Option<String>,
    /// Its description.
    #[serde(default)]
    pub description: Option<String>,
    /// When it starts (a cancelled event may have none).
    #[serde(default)]
    pub start: Option<EventTime>,
    /// When it ends.
    #[serde(default)]
    pub end: Option<EventTime>,
    /// RRULE, RDATE and EXDATE lines.
    #[serde(default)]
    pub recurrence: Vec<String>,
    /// An exception: the id of its series.
    #[serde(default)]
    pub recurring_event_id: Option<String>,
    /// An exception: the start of the occurrence it replaces.
    #[serde(default)]
    pub original_start_time: Option<EventTime>,
    /// Its iCalendar UID.
    #[serde(default, rename = "iCalUID")]
    pub ical_uid: Option<String>,
    /// Who organises it.
    #[serde(default)]
    pub organizer: Option<Person>,
    /// Who is invited.
    #[serde(default)]
    pub attendees: Vec<Person>,
    /// What apps keep on it.
    #[serde(default)]
    pub extended_properties: Option<ExtendedProperties>,
    /// `default`, `outOfOffice`, `focusTime`, `workingLocation`, …
    #[serde(default)]
    pub event_type: Option<String>,
}

/// One page of `events.list`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsPage {
    /// The events.
    #[serde(default)]
    pub items: Vec<GoogleEvent>,
    /// The next page.
    #[serde(default)]
    pub next_page_token: Option<String>,
    /// The sync token to read changes from, on the last page.
    #[serde(default)]
    pub next_sync_token: Option<String>,
}

/// What `events.list` is asked.
#[derive(Debug, Clone, Copy, Default)]
pub struct EventsQuery<'a> {
    /// Read changes since this token (no other filter may be given).
    pub sync_token: Option<&'a str>,
    /// First read: events that end after this RFC 3339 time.
    pub time_min: Option<&'a str>,
    /// The page to read.
    pub page_token: Option<&'a str>,
    /// Events per page.
    pub max: u32,
}

impl<'a> Calendar<'a> {
    pub(super) fn new(http: &'a reqwest::Client, base: &'a Url, token: &'a str) -> Self {
        Self { http, base, token }
    }

    /// The URL of `segments` under the API base; each is encoded as one path
    /// segment (calendar ids hold `@` and `#`).
    fn url(&self, segments: &[&str]) -> Result<Url, CalendarError> {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .map_err(|()| {
                CalendarError::Provider(ProviderError::Unexpected("Calendar URL".into()))
            })?
            .pop_if_empty()
            .extend(segments);
        Ok(url)
    }

    async fn send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, CalendarError> {
        let response = request
            .bearer_auth(self.token)
            .send()
            .await
            .map_err(|e| CalendarError::Provider(transport(e)))?;
        if response.status().is_success() {
            Ok(response)
        } else {
            Err(failure(response).await)
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        url: Url,
        query: &[(&str, &str)],
    ) -> Result<T, CalendarError> {
        let response = self.send(self.http.get(with_query(url, query))).await?;
        read(response).await
    }

    /// `calendarList.list`, every page.
    pub async fn calendars(&self) -> Result<Vec<CalendarEntry>, CalendarError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Page {
            #[serde(default)]
            items: Vec<CalendarEntry>,
            #[serde(default)]
            next_page_token: Option<String>,
        }
        let mut all = Vec::new();
        let mut token: Option<String> = None;
        // A bounded walk: 20 pages of 250 is far more calendars than anyone has.
        for _ in 0..20 {
            let mut query = vec![("maxResults", "250"), ("showDeleted", "false")];
            if let Some(token) = token.as_deref() {
                query.push(("pageToken", token));
            }
            let page: Page = self
                .get(self.url(&["users", "me", "calendarList"])?, &query)
                .await?;
            all.extend(page.items.into_iter().filter(|c| !c.deleted));
            token = page.next_page_token;
            if token.is_none() {
                break;
            }
        }
        Ok(all)
    }

    /// One page of `events.list` for `calendar`: series once
    /// (`singleEvents=false`), deleted events included.
    pub async fn events(
        &self,
        calendar: &str,
        query: &EventsQuery<'_>,
    ) -> Result<EventsPage, CalendarError> {
        let max = query.max.to_string();
        let mut params: Vec<(&str, &str)> = vec![
            ("singleEvents", "false"),
            ("showDeleted", "true"),
            ("maxResults", max.as_str()),
        ];
        if let Some(token) = query.sync_token {
            params.push(("syncToken", token));
        }
        if let Some(min) = query.time_min {
            params.push(("timeMin", min));
        }
        if let Some(page) = query.page_token {
            params.push(("pageToken", page));
        }
        self.get(self.url(&["calendars", calendar, "events"])?, &params)
            .await
    }

    /// `events.get`.
    pub async fn event(&self, calendar: &str, id: &str) -> Result<GoogleEvent, CalendarError> {
        self.get(self.url(&["calendars", calendar, "events", id])?, &[])
            .await
    }

    /// `events.get` as Google sent it, for a change that must send back
    /// fields this module does not read (an answer to an invitation sends
    /// the whole list of attendees).
    pub async fn event_raw(
        &self,
        calendar: &str,
        id: &str,
    ) -> Result<serde_json::Value, CalendarError> {
        self.get(self.url(&["calendars", calendar, "events", id])?, &[])
            .await
    }

    /// The instance of series `series` that replaces or is the occurrence
    /// that starts at `original_start` (an RFC 3339 time or a date), if the
    /// series has one.
    pub async fn instance(
        &self,
        calendar: &str,
        series: &str,
        original_start: &str,
    ) -> Result<Option<GoogleEvent>, CalendarError> {
        let page: EventsPage = self
            .get(
                self.url(&["calendars", calendar, "events", series, "instances"])?,
                &[
                    ("originalStart", original_start),
                    ("showDeleted", "false"),
                    ("maxResults", "1"),
                ],
            )
            .await?;
        Ok(page.items.into_iter().next())
    }

    /// `events.insert`: `body` is the event, with the `id` it is to have.
    /// `Exists` when that id is taken (the same create made before).
    pub async fn insert(
        &self,
        calendar: &str,
        body: &serde_json::Value,
    ) -> Result<GoogleEvent, CalendarError> {
        let response = self
            .send(
                self.http
                    .post(self.url(&["calendars", calendar, "events"])?)
                    .json(body),
            )
            .await?;
        read(response).await
    }

    /// `events.patch` of `id`, only if its etag is `if_match` (when given).
    /// `notify` tells the attendees (an answer to an invitation does).
    pub async fn patch(
        &self,
        calendar: &str,
        id: &str,
        body: &serde_json::Value,
        if_match: Option<&str>,
        notify: bool,
    ) -> Result<GoogleEvent, CalendarError> {
        let mut request = self
            .http
            .patch(with_query(
                self.url(&["calendars", calendar, "events", id])?,
                &[("sendUpdates", if notify { "all" } else { "none" })],
            ))
            .json(body);
        if let Some(etag) = if_match {
            request = request.header(reqwest::header::IF_MATCH, header_value(etag)?);
        }
        let response = self.send(request).await?;
        read(response).await
    }

    /// `events.delete` of `id`, only if its etag is `if_match` (when given).
    pub async fn delete(
        &self,
        calendar: &str,
        id: &str,
        if_match: Option<&str>,
    ) -> Result<(), CalendarError> {
        let mut request = self.http.delete(with_query(
            self.url(&["calendars", calendar, "events", id])?,
            &[("sendUpdates", "none")],
        ));
        if let Some(etag) = if_match {
            request = request.header(reqwest::header::IF_MATCH, header_value(etag)?);
        }
        self.send(request).await?;
        Ok(())
    }
}

/// `url` with `pairs` as its query.
fn with_query(mut url: Url, pairs: &[(&str, &str)]) -> Url {
    url.query_pairs_mut().extend_pairs(pairs);
    url
}

/// An etag as a header value.
fn header_value(etag: &str) -> Result<reqwest::header::HeaderValue, CalendarError> {
    reqwest::header::HeaderValue::from_str(etag)
        .map_err(|_| CalendarError::Provider(ProviderError::Unexpected("an unusable etag".into())))
}

/// A JSON answer.
async fn read<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, CalendarError> {
    response.json().await.map_err(|e| {
        CalendarError::Provider(ProviderError::Unexpected(format!(
            "Calendar answer: {}",
            e.without_url()
        )))
    })
}

/// What a non-success answer means: the reasons Google gives in the body
/// decide, and only they are read from it.
async fn failure(response: reqwest::Response) -> CalendarError {
    let status = response.status();
    let headers = response.headers().clone();
    let reasons = reasons(response).await;
    let has = |wanted: &str| reasons.iter().any(|reason| reason == wanted);
    if status == reqwest::StatusCode::FORBIDDEN
        && (has("accessNotConfigured") || has("SERVICE_DISABLED"))
    {
        return CalendarError::ApiNotEnabled;
    }
    match status {
        reqwest::StatusCode::GONE => return CalendarError::Gone,
        reqwest::StatusCode::PRECONDITION_FAILED => return CalendarError::PreconditionFailed,
        reqwest::StatusCode::CONFLICT => return CalendarError::Exists,
        _ => {}
    }
    // The calendar's own quota reasons are a slowdown like the others.
    let reason = if status == reqwest::StatusCode::FORBIDDEN && has("quotaExceeded") {
        Some("rateLimitExceeded")
    } else {
        reasons.first().map(String::as_str)
    };
    CalendarError::Provider(classify(status, &headers, reason))
}

/// The reasons of a Google error body (`errors[].reason`, `status`, and the
/// `details[].reason` of newer errors); nothing else of it is read.
async fn reasons(response: reqwest::Response) -> Vec<String> {
    #[derive(Deserialize)]
    struct Body {
        error: ErrorBody,
    }
    #[derive(Deserialize)]
    struct ErrorBody {
        #[serde(default)]
        errors: Vec<Reason>,
        #[serde(default)]
        details: Vec<Reason>,
        #[serde(default)]
        status: Option<String>,
    }
    #[derive(Deserialize)]
    struct Reason {
        #[serde(default)]
        reason: Option<String>,
    }
    let Ok(body) = response.json::<Body>().await else {
        return Vec::new();
    };
    body.error
        .errors
        .into_iter()
        .chain(body.error.details)
        .filter_map(|detail| detail.reason)
        .chain(body.error.status)
        .collect()
}

#[cfg(test)]
mod tests {
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path, query_param},
    };

    use super::*;
    use crate::{google::Google, provider::ClientSecret};

    /// A client for the fake Calendar API at `server`.
    fn google(server: &MockServer) -> Google {
        let base = Url::parse(&format!("{}/", server.uri())).unwrap();
        Google::new(
            "client".into(),
            ClientSecret::Inline(secrecy::SecretString::from("secret".to_owned())),
            crate::google::Endpoints::under(&base),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn the_calendar_list_is_read_with_its_roles() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/calendar/v3/users/me/calendarList"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [
                    {"id": "ines@example.com", "summary": "Ines", "primary": true,
                     "accessRole": "owner", "timeZone": "Europe/Lisbon",
                     "backgroundColor": "#0b8043"},
                    {"id": "holidays@example.com", "summary": "Holidays",
                     "accessRole": "reader"},
                    {"id": "gone@example.com", "deleted": true}
                ]
            })))
            .mount(&server)
            .await;
        let google = google(&server);
        let calendars = google.calendar("token").calendars().await.unwrap();
        assert_eq!(calendars.len(), 2);
        assert!(calendars[0].writable() && calendars[0].primary);
        assert!(!calendars[1].writable());
        assert_eq!(
            crate::calendar::owned_by(calendars[0].access_role.as_deref()),
            Some(true)
        );
        assert_eq!(
            crate::calendar::owned_by(calendars[1].access_role.as_deref()),
            Some(false)
        );
        assert_eq!(calendars[1].name(), "Holidays");
    }

    #[tokio::test]
    async fn a_calendar_id_is_one_path_segment() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/calendar/v3/calendars/ines@example.com/events"))
            .and(query_param("syncToken", "t1"))
            .and(query_param("singleEvents", "false"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [], "nextSyncToken": "t2"
            })))
            .mount(&server)
            .await;
        let google = google(&server);
        let page = google
            .calendar("token")
            .events(
                "ines@example.com",
                &EventsQuery {
                    sync_token: Some("t1"),
                    max: 250,
                    ..EventsQuery::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(page.next_sync_token.as_deref(), Some("t2"));
    }

    #[tokio::test]
    async fn answers_are_told_apart_by_status_and_reason() {
        let server = MockServer::start().await;
        let events = |id: &'static str| format!("/calendar/v3/calendars/c/events/{id}");
        for (id, status, body) in [
            (
                "gone",
                410,
                serde_json::json!({"error": {"errors": [{"reason": "fullSyncRequired"}]}}),
            ),
            (
                "disabled",
                403,
                serde_json::json!({"error": {"errors": [{"reason": "accessNotConfigured"}]}}),
            ),
            (
                "service",
                403,
                serde_json::json!({"error": {"status": "PERMISSION_DENIED",
                    "details": [{"reason": "SERVICE_DISABLED"}]}}),
            ),
            ("etag", 412, serde_json::json!({})),
            (
                "quota",
                403,
                serde_json::json!({"error": {"errors": [{"reason": "quotaExceeded"}]}}),
            ),
            (
                "plain",
                403,
                serde_json::json!({"error": {"errors": [{"reason": "forbidden"}]}}),
            ),
        ] {
            Mock::given(method("GET"))
                .and(path(events(id)))
                .respond_with(ResponseTemplate::new(status).set_body_json(body))
                .mount(&server)
                .await;
        }
        let google = google(&server);
        let api = google.calendar("token");
        assert_eq!(
            api.event("c", "gone").await.unwrap_err(),
            CalendarError::Gone
        );
        assert_eq!(
            api.event("c", "disabled").await.unwrap_err(),
            CalendarError::ApiNotEnabled
        );
        assert_eq!(
            api.event("c", "service").await.unwrap_err(),
            CalendarError::ApiNotEnabled
        );
        assert_eq!(
            api.event("c", "etag").await.unwrap_err(),
            CalendarError::PreconditionFailed
        );
        assert!(matches!(
            api.event("c", "quota").await.unwrap_err(),
            CalendarError::Provider(ProviderError::RateLimited { .. })
        ));
        assert!(matches!(
            api.event("c", "plain").await.unwrap_err(),
            CalendarError::Provider(ProviderError::Unexpected(_))
        ));
    }

    #[tokio::test]
    async fn a_patch_sends_the_etag_it_was_given() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/calendar/v3/calendars/c/events/e1"))
            .and(header("if-match", "\"123\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "e1", "etag": "\"456\""
            })))
            .mount(&server)
            .await;
        let google = google(&server);
        let event = google
            .calendar("token")
            .patch(
                "c",
                "e1",
                &serde_json::json!({"summary": "Dentist"}),
                Some("\"123\""),
                false,
            )
            .await
            .unwrap();
        assert_eq!(event.etag.as_deref(), Some("\"456\""));
    }
}
