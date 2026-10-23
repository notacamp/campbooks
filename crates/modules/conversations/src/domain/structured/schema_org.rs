//! schema.org markup in a message's HTML: JSON-LD (`<script
//! type="application/ld+json">`, parsed with `serde_json`) and microdata
//! (`itemscope`, `itemtype`, `itemprop`, read from the parsed tree), the
//! types the big senders emit for mail clients.
//!
//! Only the closed list of types below is read, and only their typed fields
//! (times, names of the things reserved, places, prices). The prose around
//! the markup is never read, and neither is a field such as `description`
//! that holds prose (ADR 0021). Both syntaxes become one JSON shape first,
//! so one reader serves both.

use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone as _};
use scraper::{ElementRef, Html, Selector};
use serde_json::{Map, Value};

use super::{Candidate, Due};
use crate::{
    domain::read::{Amount, MAX_PLACE, MAX_TITLE, When, tidy},
    model::ItemKind,
};

/// Most JSON-LD blocks and microdata items read from one message.
const MAX_BLOCKS: usize = 40;
/// Most objects looked at after flattening.
const MAX_OBJECTS: usize = 200;
/// Deepest nesting of microdata items followed.
const MAX_DEPTH: usize = 6;

/// The JSON-LD objects and the microdata items of `html`, as JSON.
pub fn values(html: &str) -> Vec<Value> {
    let document = Html::parse_document(html);
    let mut out = json_ld(&document);
    out.extend(microdata(&document));
    out
}

/// The text under `element`, concatenated. The only place text is taken from
/// the tree: a JSON-LD block's JSON and a microdata property's value
/// (`json_ld` and `microdata_value`; the source test keeps it so).
fn text_of(element: ElementRef<'_>) -> String {
    element.text().collect()
}

// --- JSON-LD ---------------------------------------------------------------

/// Every JSON-LD block of `document`, flattened to its objects.
fn json_ld(document: &Html) -> Vec<Value> {
    let Ok(scripts) = Selector::parse("script") else {
        return Vec::new();
    };
    let mut objects = Vec::new();
    let mut blocks = 0;
    for script in document.select(&scripts) {
        let is_json_ld = script.attr("type").is_some_and(|kind| {
            kind.split(';')
                .next()
                .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/ld+json"))
        });
        if !is_json_ld {
            continue;
        }
        blocks += 1;
        if blocks > MAX_BLOCKS {
            break;
        }
        if let Ok(value) = serde_json::from_str::<Value>(text_of(script).trim()) {
            flatten(value, &mut objects, 0);
        }
    }
    objects
}

/// `value`'s objects: those of arrays and of an `@graph`, and itself.
fn flatten(value: Value, out: &mut Vec<Value>, depth: usize) {
    if out.len() >= MAX_OBJECTS || depth > MAX_DEPTH {
        return;
    }
    match value {
        Value::Array(items) => {
            for item in items {
                flatten(item, out, depth + 1);
            }
        }
        Value::Object(mut map) => {
            if let Some(Value::Array(graph)) = map.remove("@graph") {
                for item in graph {
                    flatten(item, out, depth + 1);
                }
            }
            out.push(Value::Object(map));
        }
        _ => {}
    }
}

// --- Microdata -------------------------------------------------------------

/// The top-level microdata items of `document` (an `itemscope` that is not
/// itself a property), as JSON.
fn microdata(document: &Html) -> Vec<Value> {
    let Ok(scopes) = Selector::parse("[itemscope]") else {
        return Vec::new();
    };
    document
        .select(&scopes)
        .filter(|element| element.attr("itemprop").is_none())
        .take(MAX_BLOCKS)
        .map(|element| item(element, 0))
        .collect()
}

/// One microdata item as a JSON object: its `@type` (the last path segment of
/// its first `itemtype`) and its properties. A property's value is a string
/// ([`microdata_value`]) or, when the property is itself an item, an
/// object. The tree is walked with a stack, so nesting costs no recursion.
fn item(scope: ElementRef<'_>, depth: usize) -> Value {
    let mut map = Map::new();
    if let Some(kind) = scope
        .attr("itemtype")
        .and_then(|types| types.split_whitespace().next())
    {
        map.insert("@type".to_owned(), Value::String(local(kind).to_owned()));
    }
    let mut stack: Vec<ElementRef<'_>> = children(scope);
    while let Some(element) = stack.pop() {
        let nested = element.attr("itemscope").is_some();
        match element.attr("itemprop") {
            Some(names) => {
                let value = if nested {
                    if depth >= MAX_DEPTH {
                        continue;
                    }
                    item(element, depth + 1)
                } else {
                    Value::String(microdata_value(element))
                };
                for name in names.split_whitespace() {
                    map.entry(name.to_owned()).or_insert_with(|| value.clone());
                }
                // A property that is not an item may hold more properties of
                // this one.
                if !nested {
                    stack.extend(children(element));
                }
            }
            // Another item: its properties are its own.
            None if nested => {}
            None => stack.extend(children(element)),
        }
    }
    Value::Object(map)
}

/// The child elements of `element`, last first, so a stack pops them in
/// document order.
fn children(element: ElementRef<'_>) -> Vec<ElementRef<'_>> {
    let mut all: Vec<ElementRef<'_>> = element.child_elements().collect();
    all.reverse();
    all
}

/// The value of the property `element` carries, by the element it is (the
/// microdata rules): a `meta`'s `content`, a link's `href`, a `time`'s
/// `datetime`, a `data`'s `value`, an image's `src`, else its text.
fn microdata_value(element: ElementRef<'_>) -> String {
    let attribute = match element.value().name() {
        "meta" => Some("content"),
        "audio" | "embed" | "iframe" | "img" | "source" | "track" | "video" => Some("src"),
        "a" | "area" | "link" => Some("href"),
        "object" => Some("data"),
        "data" | "meter" => Some("value"),
        "time" => Some("datetime"),
        _ => None,
    };
    attribute
        .and_then(|name| element.attr(name))
        .map_or_else(|| text_of(element), str::to_owned)
        .trim()
        .to_owned()
}

// --- Reading the typed fields ------------------------------------------------

/// The things markup says about the reader's dated things and payments.
pub fn interpret(values: &[Value]) -> (Vec<Candidate>, Vec<Due>) {
    let mut items = Vec::new();
    let mut asks = Vec::new();
    for value in values {
        let Some(object) = value.as_object() else {
            continue;
        };
        for kind in types(object) {
            match kind.as_str() {
                "FlightReservation" => items.extend(trip(
                    object,
                    ItemKind::Travel,
                    &Trip {
                        from: &["departureAirport"],
                        to: &["arrivalAirport"],
                        number: "flightNumber",
                        carrier: Some("airline"),
                    },
                )),
                "TrainReservation" => items.extend(trip(
                    object,
                    ItemKind::Travel,
                    &Trip {
                        from: &["departureStation"],
                        to: &["arrivalStation"],
                        number: "trainNumber",
                        carrier: Some("provider"),
                    },
                )),
                "BusReservation" => items.extend(trip(
                    object,
                    ItemKind::Travel,
                    &Trip {
                        from: &["departureBusStop", "departureStation"],
                        to: &["arrivalBusStop", "arrivalStation"],
                        number: "busNumber",
                        carrier: Some("provider"),
                    },
                )),
                "RentalCarReservation" => items.extend(rental_car(object)),
                "LodgingReservation" => items.extend(lodging(object)),
                "EventReservation" => items.extend(event(object)),
                "FoodEstablishmentReservation" => items.extend(restaurant(object)),
                "ParcelDelivery" => items.extend(parcel(object)),
                "Invoice" => asks.extend(invoice(object)),
                _ => {}
            }
        }
    }
    (items, asks)
}

/// The local names of an object's `@type` (a string or a list), such as
/// `FlightReservation` for `http://schema.org/FlightReservation`.
fn types(object: &Map<String, Value>) -> Vec<String> {
    match object.get("@type") {
        Some(Value::String(kind)) => vec![local(kind).to_owned()],
        Some(Value::Array(kinds)) => kinds
            .iter()
            .filter_map(Value::as_str)
            .map(|kind| local(kind).to_owned())
            .collect(),
        _ => Vec::new(),
    }
}

/// The last segment of a type name or URL.
fn local(name: &str) -> &str {
    name.trim().rsplit(['/', ':', '#']).next().unwrap_or(name)
}

/// The value of `key`, the first of a list.
fn field<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    match object.get(key)? {
        Value::Array(list) => list.first(),
        other => Some(other),
    }
}

/// `key` as an object.
fn object<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a Map<String, Value>> {
    field(object, key)?.as_object()
}

/// `key` as text: a string, or the `name` of an object.
fn text(object: &Map<String, Value>, key: &str) -> Option<String> {
    let value = field(object, key)?;
    let raw = match value {
        Value::String(text) => text.as_str(),
        Value::Number(_) => return Some(value.to_string()),
        Value::Object(inner) => inner.get("name")?.as_str()?,
        _ => return None,
    };
    let line = tidy(raw, MAX_TITLE);
    (!line.is_empty()).then_some(line)
}

/// The name of the thing `object` is.
fn name(object: &Map<String, Value>) -> Option<String> {
    text(object, "name")
}

/// A place from an address: a string, or the street and the town of a
/// `PostalAddress`.
fn address(thing: &Map<String, Value>) -> Option<String> {
    match field(thing, "address")? {
        Value::String(line) => Some(tidy(line, MAX_PLACE)).filter(|line| !line.is_empty()),
        Value::Object(postal) => address_of_postal(postal),
        _ => None,
    }
}

/// `key` as a moment: an ISO 8601 time with an offset or a date.
fn moment(object: &Map<String, Value>, key: &str) -> Option<When> {
    iso(field(object, key)?.as_str()?)
}

/// An ISO 8601 time with an offset (`Z`, `+01:00`, `+0100`, `+01`; seconds
/// optional) or a date. A time with no offset names no instant: left out.
fn iso(text: &str) -> Option<When> {
    let text = text.trim();
    if let Some(when) = When::parse(text) {
        return Some(when);
    }
    let split = text.find('T')?;
    let (local_time, offset) = if let Some(local_time) = text.strip_suffix(['Z', 'z']) {
        (local_time, "+00:00".to_owned())
    } else {
        let sign = text[split..].rfind(['+', '-'])? + split;
        (&text[..sign], text[sign..].to_owned())
    };
    let naive = NaiveDateTime::parse_from_str(local_time, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(local_time, "%Y-%m-%dT%H:%M"))
        .ok()?;
    let digits: String = offset[1..].chars().filter(char::is_ascii_digit).collect();
    let (hours, minutes) = match digits.len() {
        2 => (digits.parse::<i32>().ok()?, 0),
        4 => (
            digits[..2].parse::<i32>().ok()?,
            digits[2..].parse::<i32>().ok()?,
        ),
        _ => return None,
    };
    let seconds = hours * 3600 + minutes * 60;
    let zone = if offset.starts_with('-') {
        FixedOffset::west_opt(seconds)?
    } else {
        FixedOffset::east_opt(seconds)?
    };
    let time: DateTime<FixedOffset> = zone.from_local_datetime(&naive).single()?;
    Some(When::Time(time))
}

/// What a trip reservation names.
struct Trip {
    /// Keys of the place it leaves from, in order of preference.
    from: &'static [&'static str],
    /// Keys of the place it arrives at.
    to: &'static [&'static str],
    /// Key of its number on the trip.
    number: &'static str,
    /// Key of its carrier, whose code leads the number.
    carrier: Option<&'static str>,
}

/// A trip's end: the code or the name of the place, from the first of
/// `keys` the trip has.
fn endpoint(trip: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        let place = object(trip, key)?;
        text(place, "iataCode").or_else(|| name(place))
    })
}

fn trip(reservation: &Map<String, Value>, kind: ItemKind, shape: &Trip) -> Option<Candidate> {
    let journey = object(reservation, "reservationFor")?;
    let starts = moment(journey, "departureTime")?;
    let ends = moment(journey, "arrivalTime");
    let from = endpoint(journey, shape.from);
    let to = endpoint(journey, shape.to);
    let number = text(journey, shape.number);
    let carrier = shape
        .carrier
        .and_then(|key| object(journey, key))
        .and_then(|carrier| text(carrier, "iataCode"));
    let designator = match (carrier, number) {
        (Some(carrier), Some(number)) => Some(format!("{carrier}{number}")),
        (None, Some(number)) => Some(number),
        _ => None,
    };
    let route = match (from, to) {
        (Some(from), Some(to)) => Some(format!("{from} → {to}")),
        _ => None,
    };
    let title = [designator, route]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    candidate(kind, &title, starts, ends, None)
}

fn rental_car(reservation: &Map<String, Value>) -> Option<Candidate> {
    let starts = moment(reservation, "pickupTime")?;
    let ends = moment(reservation, "dropoffTime");
    let title = object(reservation, "reservationFor")
        .and_then(|car| {
            object(car, "rentalCompany")
                .and_then(name)
                .or_else(|| name(car))
        })
        .or_else(|| object(reservation, "provider").and_then(name))?;
    let place = object(reservation, "pickupLocation").and_then(name);
    candidate(ItemKind::Travel, &title, starts, ends, place)
}

fn lodging(reservation: &Map<String, Value>) -> Option<Candidate> {
    let starts =
        moment(reservation, "checkinTime").or_else(|| moment(reservation, "checkinDate"))?;
    let ends = moment(reservation, "checkoutTime").or_else(|| moment(reservation, "checkoutDate"));
    let place = object(reservation, "reservationFor")?;
    let title = name(place)?;
    candidate(ItemKind::Stay, &title, starts, ends, address(place))
}

fn event(reservation: &Map<String, Value>) -> Option<Candidate> {
    let show = object(reservation, "reservationFor")?;
    let starts = moment(show, "startDate")?;
    let ends = moment(show, "endDate");
    let place = object(show, "location").and_then(|venue| name(venue).or_else(|| address(venue)));
    candidate(ItemKind::Show, &name(show)?, starts, ends, place)
}

fn restaurant(reservation: &Map<String, Value>) -> Option<Candidate> {
    let table = object(reservation, "reservationFor")?;
    let starts = moment(reservation, "startTime")?;
    let ends = moment(reservation, "endTime");
    candidate(
        ItemKind::Reservation,
        &name(table)?,
        starts,
        ends,
        address(table),
    )
}

fn parcel(delivery: &Map<String, Value>) -> Option<Candidate> {
    let from = moment(delivery, "expectedArrivalFrom");
    let until = moment(delivery, "expectedArrivalUntil");
    let (starts, ends) = match (from, until) {
        (Some(from), until) => (from, until),
        (None, Some(until)) => (until, None),
        (None, None) => return None,
    };
    let title = object(delivery, "provider")
        .and_then(name)
        .or_else(|| {
            object(delivery, "partOfOrder")
                .and_then(|order| object(order, "seller"))
                .and_then(name)
        })
        .or_else(|| text(delivery, "trackingNumber"))?;
    let place = object(delivery, "deliveryAddress").and_then(address_of_postal);
    candidate(ItemKind::Delivery, &title, starts, ends, place)
}

/// A `PostalAddress` given directly (a delivery's), as a place.
fn address_of_postal(postal: &Map<String, Value>) -> Option<String> {
    let parts: Vec<String> = ["streetAddress", "addressLocality"]
        .iter()
        .filter_map(|key| text(postal, key))
        .collect();
    let joined = tidy(&parts.join(", "), MAX_PLACE);
    (!joined.is_empty()).then_some(joined)
}

/// An invoice with a payment due date: a payment to make, unless it is
/// marked paid.
fn invoice(bill: &Map<String, Value>) -> Option<Due> {
    let paid =
        text(bill, "paymentStatus").is_some_and(|status| local(&status) == "PaymentComplete");
    if paid {
        return None;
    }
    let due = moment(bill, "paymentDueDate").or_else(|| moment(bill, "paymentDue"))?;
    let title = object(bill, "provider")
        .and_then(name)
        .or_else(|| text(bill, "accountId"))
        .or_else(|| text(bill, "confirmationNumber"))?;
    let amount = object(bill, "totalPaymentDue").and_then(|total| {
        let price = field(total, "price")?;
        let digits = match price {
            Value::String(written) => written.trim().to_owned(),
            Value::Number(number) => number.to_string(),
            _ => return None,
        };
        let currency = text(total, "priceCurrency")?;
        Some(Amount {
            minor: minor_units(&digits)?,
            currency: currency.to_ascii_uppercase(),
        })
        .filter(|amount| amount.currency.len() == 3 && amount.minor > 0)
    });
    Some(Due {
        title,
        by: due,
        amount,
    })
}

/// A decimal price (`120`, `120.5`, `9.99`) in minor units; more than two
/// decimals, a sign or anything but digits is refused.
fn minor_units(price: &str) -> Option<i64> {
    let (whole, fraction) = price.split_once('.').unwrap_or((price, ""));
    let only_digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
    if whole.is_empty()
        || whole.len() > 9
        || fraction.len() > 2
        || !only_digits(whole)
        || !only_digits(fraction)
    {
        return None;
    }
    let cents = format!("{fraction:0<2}");
    Some(whole.parse::<i64>().ok()? * 100 + cents.parse::<i64>().ok()?)
}

/// A dated thing, when it has a title.
fn candidate(
    kind: ItemKind,
    title: &str,
    starts: When,
    ends: Option<When>,
    place: Option<String>,
) -> Option<Candidate> {
    let title = tidy(title, MAX_TITLE);
    if title.is_empty() {
        return None;
    }
    let ends = ends.filter(|ends| ends.instant() > starts.instant());
    Some(Candidate {
        kind,
        title,
        starts,
        ends,
        time_zone: None,
        place: place
            .map(|place| tidy(&place, MAX_PLACE))
            .filter(|place| !place.is_empty()),
        uid: None,
        sequence: 0,
        cancelled: false,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn read(html: &str) -> (Vec<Candidate>, Vec<Due>) {
        interpret(&values(html))
    }

    const FLIGHT: &str = r#"<html><head><script type="application/ld+json">
    {"@context": "http://schema.org", "@type": "FlightReservation",
     "reservationNumber": "RXJ34P",
     "reservationFor": {"@type": "Flight", "flightNumber": "1234",
        "airline": {"@type": "Airline", "name": "Air Example", "iataCode": "AE"},
        "departureAirport": {"@type": "Airport", "name": "Lisbon", "iataCode": "LIS"},
        "departureTime": "2026-11-06T07:40:00+00:00",
        "arrivalAirport": {"@type": "Airport", "name": "Amsterdam", "iataCode": "AMS"},
        "arrivalTime": "2026-11-06T11:55:00+01:00"}}
    </script></head><body><p>Your flight leaves on the 7th of November at noon.</p></body></html>"#;

    #[test]
    fn a_json_ld_flight_is_travel_with_its_route_and_times() {
        let (items, asks) = read(FLIGHT);
        assert!(asks.is_empty());
        assert_eq!(items.len(), 1);
        let flight = &items[0];
        assert_eq!(flight.kind, ItemKind::Travel);
        assert_eq!(flight.title, "AE1234 LIS → AMS");
        // The typed time, not the prose beside it.
        assert_eq!(flight.starts.to_text(), "2026-11-06T07:40:00+00:00");
        assert_eq!(
            flight.ends.map(When::to_text).as_deref(),
            Some("2026-11-06T11:55:00+01:00")
        );
        assert_eq!(flight.uid, None);
    }

    #[test]
    fn microdata_reads_like_json_ld() {
        let html = r#"<div itemscope itemtype="http://schema.org/LodgingReservation">
          <meta itemprop="checkinDate" content="2026-11-06">
          <time itemprop="checkoutDate" datetime="2026-11-08">Sunday</time>
          <div itemprop="reservationFor" itemscope itemtype="http://schema.org/LodgingBusiness">
            <span itemprop="name">Hotel Alfama</span>
            <div itemprop="address" itemscope itemtype="http://schema.org/PostalAddress">
              <span itemprop="streetAddress">Rua do Salvador 1</span>,
              <span itemprop="addressLocality">Lisboa</span>
            </div>
          </div>
        </div>"#;
        let (items, _) = read(html);
        assert_eq!(items.len(), 1);
        let stay = &items[0];
        assert_eq!(stay.kind, ItemKind::Stay);
        assert_eq!(stay.title, "Hotel Alfama");
        assert_eq!(stay.place.as_deref(), Some("Rua do Salvador 1, Lisboa"));
        assert_eq!(stay.starts.to_text(), "2026-11-06");
        assert!(stay.starts.is_day());
        assert_eq!(stay.ends.map(When::to_text).as_deref(), Some("2026-11-08"));
    }

    #[test]
    fn every_listed_type_has_its_kind() {
        let cases = [
            (
                json!({"@type": "EventReservation", "reservationFor": {"@type": "Event",
                    "name": "Mariza at Coliseu", "startDate": "2026-12-04T21:00:00+00:00",
                    "location": {"@type": "Place", "name": "Coliseu dos Recreios"}}}),
                ItemKind::Show,
                "Mariza at Coliseu",
            ),
            (
                json!({"@type": "FoodEstablishmentReservation",
                    "startTime": "2026-11-12T20:00:00+00:00",
                    "reservationFor": {"@type": "FoodEstablishment", "name": "Taberna do Largo"}}),
                ItemKind::Reservation,
                "Taberna do Largo",
            ),
            (
                json!({"@type": "TrainReservation", "reservationFor": {"@type": "TrainTrip",
                    "trainNumber": "IC 523", "departureTime": "2026-11-09T08:00:00+00:00",
                    "departureStation": {"name": "Lisboa Oriente"},
                    "arrivalStation": {"name": "Porto Campanha"}}}),
                ItemKind::Travel,
                "IC 523 Lisboa Oriente → Porto Campanha",
            ),
            (
                json!({"@type": "RentalCarReservation", "pickupTime": "2026-11-10T09:00:00Z",
                    "dropoffTime": "2026-11-13T09:00:00Z",
                    "reservationFor": {"@type": "Car", "name": "Compact car",
                        "rentalCompany": {"name": "Rent Example"}}}),
                ItemKind::Travel,
                "Rent Example",
            ),
            (
                json!({"@type": "ParcelDelivery",
                    "expectedArrivalFrom": "2026-11-05T09:00:00+00:00",
                    "expectedArrivalUntil": "2026-11-05T13:00:00+00:00",
                    "provider": {"@type": "Organization", "name": "Lojas Sol"}}),
                ItemKind::Delivery,
                "Lojas Sol",
            ),
        ];
        for (markup, kind, title) in cases {
            let (items, asks) = interpret(std::slice::from_ref(&markup));
            assert!(asks.is_empty(), "{markup}");
            assert_eq!(items.len(), 1, "{markup}");
            assert_eq!(
                (items[0].kind, items[0].title.as_str()),
                (kind, title),
                "{markup}"
            );
        }
    }

    #[test]
    fn an_invoice_with_a_due_date_is_a_payment_with_its_total() {
        let bill = json!({"@type": "Invoice", "provider": {"@type": "Organization",
            "name": "Águas do Vale"}, "paymentDueDate": "2026-11-30",
            "totalPaymentDue": {"@type": "PriceSpecification", "price": "47.30",
                "priceCurrency": "eur"}});
        let (items, asks) = interpret(&[bill]);
        assert!(items.is_empty());
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0].title, "Águas do Vale");
        assert_eq!(asks[0].by.to_text(), "2026-11-30");
        assert_eq!(
            asks[0].amount,
            Some(Amount {
                minor: 4730,
                currency: "EUR".into()
            })
        );
        // Paid, or no due date: nothing to pay.
        let paid = json!({"@type": "Invoice", "provider": {"name": "Águas do Vale"},
            "paymentDueDate": "2026-11-30", "paymentStatus": "http://schema.org/PaymentComplete"});
        assert!(interpret(&[paid]).1.is_empty());
        let undated = json!({"@type": "Invoice", "provider": {"name": "Águas do Vale"}});
        assert!(interpret(&[undated]).1.is_empty());
    }

    #[test]
    fn other_types_and_prose_fields_are_ignored() {
        let unknown = json!({"@type": "Product", "name": "Sneakers",
            "offers": {"validThrough": "2026-11-30"}});
        assert_eq!(interpret(&[unknown]), (Vec::new(), Vec::new()));
        // A listed type with only prose where times belong names nothing.
        let prose = json!({"@type": "FlightReservation", "reservationFor": {
            "flightNumber": "1", "departureTime": "next Friday morning",
            "description": "Departs 6 November at 07:40"}});
        assert!(interpret(&[prose]).0.is_empty());
        // No markup, no dates: the prose is not read.
        let (items, asks) = read("<p>Your flight leaves on 6 November at 07:40 sharp.</p>");
        assert!(items.is_empty() && asks.is_empty());
    }

    #[test]
    fn a_time_with_no_offset_names_no_instant() {
        for floating in ["2026-11-06T07:40:00", "2026-11-06T07:40", "07:40"] {
            assert_eq!(iso(floating), None, "{floating}");
        }
        for (written, instant) in [
            ("2026-11-06T07:40Z", "2026-11-06T07:40:00+00:00"),
            ("2026-11-06T07:40:00.5+0100", "2026-11-06T07:40:00+01:00"),
            ("2026-11-06T07:40:00-05", "2026-11-06T07:40:00-05:00"),
            ("2026-11-06T07:40+01:00", "2026-11-06T07:40:00+01:00"),
        ] {
            assert_eq!(
                iso(written).map(When::to_text).as_deref(),
                Some(instant),
                "{written}"
            );
        }
    }

    #[test]
    fn graphs_lists_and_microdata_property_rules() {
        let html = r#"<script type="application/ld+json; charset=utf-8">
          {"@graph": [{"@type": ["Thing", "http://schema.org/Invoice"], "provider": {"name": "ISP"},
            "paymentDueDate": "2026-11-20"}]}</script>
          <script type="text/javascript">{"@type": "Invoice", "paymentDueDate": "2026-01-01"}</script>
          <script type="application/ld+json">{ not json</script>"#;
        let (_, asks) = read(html);
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0].by.to_text(), "2026-11-20");
        // Properties of a nested item are the nested item's, not the outer's.
        let nested = r#"<div itemscope itemtype="https://schema.org/EventReservation">
          <div itemprop="reservationFor" itemscope itemtype="https://schema.org/Event">
            <span itemprop="name">Concert</span>
            <meta itemprop="startDate" content="2026-12-04T21:00:00+00:00">
          </div></div>"#;
        let (items, _) = read(nested);
        assert_eq!(items[0].title, "Concert");
    }

    #[test]
    fn prices_are_decimals_of_at_most_two_places() {
        assert_eq!(minor_units("120"), Some(12_000));
        assert_eq!(minor_units("9.99"), Some(999));
        assert_eq!(minor_units("120.5"), Some(12_050));
        for bad in ["", "-5", "1.234", "1,50", "€5", "1e3", "12345678901"] {
            assert_eq!(minor_units(bad), None, "{bad}");
        }
    }
}
