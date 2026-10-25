//! The workbook: its sheets and columns, reading it (calamine) and writing
//! it in canonical form (`rust_xlsxwriter`).
//!
//! Reading compares parsed content only, so a workbook saved by any
//! spreadsheet application reads the same as the one the tool wrote. Writing is
//! byte-stable: rows in canonical order, fixed styles, and the document's
//! creation date set to the plan version's date.

use std::path::Path;

use anyhow::{Context as _, Result};
use calamine::{Data, Reader as _, Xlsx, open_workbook};
use rust_xlsxwriter::{
    Color, DataValidation, DocProperties, ExcelDateTime, Format, FormatAlign, FormatBorder,
    Workbook, Worksheet,
};

use crate::model::{
    ActorRow, CLASSIFICATIONS, COVERAGE_KINDS, ChangelogRow, CoverageRow, EnumRow, EventRow, Plan,
    PropertyRow, RecordRow, SCOPES, STATUSES, TYPES, YES_NO,
};

/// One column of a sheet.
#[derive(Debug, Clone, Copy)]
pub struct Column {
    /// The header cell's text.
    pub header: &'static str,
    /// Its width, in characters.
    pub width: f64,
    /// The values its dropdown offers, when it has one.
    pub dropdown: Option<&'static [&'static str]>,
    /// What a cell holds, shown when it is selected.
    pub help: &'static str,
}

const fn column(header: &'static str, width: f64, help: &'static str) -> Column {
    Column {
        header,
        width,
        dropdown: None,
        help,
    }
}

const fn dropdown(
    header: &'static str,
    width: f64,
    values: &'static [&'static str],
    help: &'static str,
) -> Column {
    Column {
        header,
        width,
        dropdown: Some(values),
        help,
    }
}

/// The README sheet's name.
pub const README: &str = "README";

/// The data sheets, in workbook order (after the README).
pub const SHEETS: &[(&str, &[Column])] = &[
    (EVENTS, EVENT_COLUMNS),
    (PROPERTIES, PROPERTY_COLUMNS),
    (RECORDS, RECORD_COLUMNS),
    (ENUMS, ENUM_COLUMNS),
    (ACTORS, ACTOR_COLUMNS),
    (COVERAGE, COVERAGE_COLUMNS),
    (CHANGELOG, CHANGELOG_COLUMNS),
];

const EVENTS: &str = "Events";
const PROPERTIES: &str = "Properties";
const RECORDS: &str = "Records";
const ENUMS: &str = "Enums";
const ACTORS: &str = "Actors";
const COVERAGE: &str = "Coverage";
const CHANGELOG: &str = "Changelog";

const EVENT_COLUMNS: &[Column] = &[
    column("Event", 40.0, "<module>.<object>_<past verb>"),
    column(
        "Version",
        9.0,
        "From 1. A published version never changes meaning.",
    ),
    column(
        "Module",
        16.0,
        "The owning module (or kernel, server, bff).",
    ),
    column("Object", 18.0, "What the event is about."),
    column("Action", 14.0, "What happened to it, in the past tense."),
    dropdown("Scope", 12.0, SCOPES, "workspace or instance"),
    dropdown("Status", 12.0, STATUSES, "planned, live or deprecated"),
    column(
        "Actor types",
        30.0,
        "Comma-separated: who may cause it (see Actors).",
    ),
    column("Description", 60.0, "What the event records."),
    column("Fires when", 60.0, "When exactly it is tracked."),
    column(
        "Added in plan version",
        12.0,
        "The plan version that added it.",
    ),
    column("OpenSpec change", 28.0, "The change that added it."),
];

const PROPERTY_COLUMNS: &[Column] = &[
    column("Event", 40.0, "The event's name."),
    column("Version", 9.0, "The event's version."),
    column("Property", 24.0, "snake_case"),
    dropdown("Type", 14.0, TYPES, "There is no free-text type."),
    dropdown(
        "List",
        8.0,
        YES_NO,
        "yes for a list of values (never of records).",
    ),
    column(
        "Enum or record",
        30.0,
        "The enum (type enum) or record (type record) it holds.",
    ),
    dropdown("Required", 10.0, YES_NO, "yes when always present."),
    dropdown(
        "Classification",
        16.0,
        CLASSIFICATIONS,
        "personal and content are rejected.",
    ),
    column("Pattern", 24.0, "The pattern a code matches (code only)."),
    column("Description", 60.0, "What it holds."),
    column("Example", 30.0, "An example value."),
];

const RECORD_COLUMNS: &[Column] = &[
    column("Record", 32.0, "<module>.<record>"),
    column("Field", 24.0, "snake_case"),
    dropdown("Type", 14.0, TYPES, "Any type but record."),
    dropdown("List", 8.0, YES_NO, "yes for a list of values."),
    column("Enum", 30.0, "The enum it holds (type enum)."),
    dropdown("Required", 10.0, YES_NO, "yes when always present."),
    dropdown(
        "Classification",
        16.0,
        CLASSIFICATIONS,
        "personal and content are rejected.",
    ),
    column("Pattern", 24.0, "The pattern a code matches (code only)."),
    column("Description", 60.0, "What it holds."),
];

const ENUM_COLUMNS: &[Column] = &[
    column("Enum", 32.0, "<module>.<enum>"),
    column("Value", 24.0, "snake_case"),
    column("Description", 60.0, "What it means."),
];

const ACTOR_COLUMNS: &[Column] = &[
    column("Actor type", 14.0, "Fixed: these seven rows."),
    column("The id is", 40.0, "What the actor's id holds."),
    column("Description", 70.0, "Who it is."),
];

const COVERAGE_COLUMNS: &[Column] = &[
    dropdown(
        "Kind",
        12.0,
        COVERAGE_KINDS,
        "operation, job, schedule or handler",
    ),
    column(
        "Name",
        40.0,
        "The operationId, or the job, schedule or handler kind.",
    ),
    column("Events", 50.0, "Comma-separated event names it tracks."),
    column(
        "Exemption",
        60.0,
        "Why it tracks nothing (instead of events).",
    ),
];

const CHANGELOG_COLUMNS: &[Column] = &[
    column("Date", 12.0, "YYYY-MM-DD"),
    column("Plan version", 12.0, "From 1, one per change."),
    column("Change", 70.0, "What changed."),
    column("PR", 10.0, "#123"),
];

/// Read the plan from the workbook at `path`. Problems with the workbook's
/// shape (a missing sheet, a renamed header, a version that is not a
/// number) are returned together, each naming its sheet and cell.
pub fn read(path: &Path) -> Result<Result<Plan, Vec<String>>> {
    let mut workbook: Xlsx<_> =
        open_workbook(path).with_context(|| format!("opening {}", path.display()))?;
    let mut problems = Vec::new();
    let names = workbook.sheet_names();
    let expected: Vec<&str> = std::iter::once(README)
        .chain(SHEETS.iter().map(|(name, _)| *name))
        .collect();
    for name in &names {
        if !expected.contains(&name.as_str()) {
            problems.push(format!(
                "sheet `{name}` is not part of the plan (sheets: {})",
                expected.join(", ")
            ));
        }
    }
    let mut plan = Plan::default();
    for (sheet, columns) in SHEETS {
        if !names.iter().any(|name| name == sheet) {
            problems.push(format!("sheet `{sheet}` is missing"));
            continue;
        }
        let range = workbook
            .worksheet_range(sheet)
            .with_context(|| format!("reading sheet `{sheet}`"))?;
        let rows = table(&range, sheet, columns, &mut problems);
        for (line, cells) in rows {
            let mut at = Cells {
                sheet,
                line,
                values: &cells,
                problems: &mut problems,
            };
            match *sheet {
                EVENTS => plan.events.push(at.event()),
                PROPERTIES => plan.properties.push(at.property()),
                RECORDS => plan.records.push(at.record()),
                ENUMS => plan.enums.push(at.enumeration()),
                ACTORS => plan.actors.push(at.actor()),
                COVERAGE => plan.coverage.push(at.coverage()),
                _ => plan.changelog.push(at.changelog()),
            }
        }
    }
    if problems.is_empty() {
        Ok(Ok(plan))
    } else {
        Ok(Err(problems))
    }
}

/// The data rows of a sheet (1-based spreadsheet line, trimmed cells),
/// after checking its header row.
fn table(
    range: &calamine::Range<Data>,
    sheet: &str,
    columns: &[Column],
    problems: &mut Vec<String>,
) -> Vec<(u32, Vec<String>)> {
    let Some((end_row, end_col)) = range.end() else {
        problems.push(format!(
            "sheet `{sheet}` is empty: its header row is missing"
        ));
        return Vec::new();
    };
    let cell = |row: u32, col: u32| range.get_value((row, col)).map(text).unwrap_or_default();
    let width = u32::try_from(columns.len()).unwrap_or(u32::MAX);
    for (col, column) in (0..).zip(columns) {
        let found = cell(0, col);
        if found != column.header {
            problems.push(format!(
                "sheet `{sheet}`: header {} is `{found}`, expected `{}`",
                cell_name(0, col),
                column.header
            ));
        }
    }
    for col in width..=end_col {
        let found = cell(0, col);
        if !found.is_empty() {
            problems.push(format!(
                "sheet `{sheet}`: column {} (`{found}`) is not part of the plan",
                cell_name(0, col)
            ));
        }
    }
    let mut rows = Vec::new();
    for row in 1..=end_row {
        let cells: Vec<String> = (0..width).map(|col| cell(row, col)).collect();
        let extra = (width..=end_col).any(|col| !cell(row, col).is_empty());
        if extra {
            problems.push(format!(
                "sheet `{sheet}` line {}: a cell right of the last column holds data",
                row + 1
            ));
        }
        if cells.iter().all(String::is_empty) {
            continue;
        }
        rows.push((row + 1, cells));
    }
    rows
}

/// A cell's text, trimmed: numbers without a fraction as integers, a
/// boolean as yes/no, a date as `YYYY-MM-DD`.
fn text(data: &Data) -> String {
    match data {
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{f:.0}"),
        Data::Bool(b) => if *b { "yes" } else { "no" }.to_owned(),
        Data::DateTime(when) => serial_to_date(when.as_f64()),
        Data::DateTimeIso(iso) => iso.get(..10).unwrap_or(iso).to_owned(),
        other => other.to_string().trim().to_owned(),
    }
}

/// An Excel serial date (days since 1899-12-30) as `YYYY-MM-DD`.
fn serial_to_date(serial: f64) -> String {
    // Days from 1970-01-01, then Howard Hinnant's civil-from-days.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a spreadsheet date is far inside i64"
    )]
    let since_epoch = serial.floor() as i64 - 25_569;
    let shifted = since_epoch + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// `A1`-style name of a 0-based cell.
fn cell_name(row: u32, col: u32) -> String {
    let mut letters = String::new();
    let mut n = col + 1;
    while n > 0 {
        let rem = (n - 1) % 26;
        letters.insert(0, char::from(b'A' + u8::try_from(rem).unwrap_or(0)));
        n = (n - 1) / 26;
    }
    format!("{letters}{}", row + 1)
}

/// One data row being read.
struct Cells<'a> {
    sheet: &'a str,
    line: u32,
    values: &'a [String],
    problems: &'a mut Vec<String>,
}

impl Cells<'_> {
    fn text(&self, col: usize) -> String {
        self.values.get(col).cloned().unwrap_or_default()
    }

    fn number<T>(&mut self, col: usize, what: &str) -> T
    where
        T: std::str::FromStr + Default,
    {
        let raw = self.text(col);
        raw.parse().unwrap_or_else(|_| {
            self.problems.push(format!(
                "sheet `{}` line {}: {what} `{raw}` is not a whole number",
                self.sheet, self.line
            ));
            T::default()
        })
    }

    fn yes_no(&mut self, col: usize, what: &str) -> bool {
        match self.text(col).to_ascii_lowercase().as_str() {
            "yes" | "true" => true,
            "no" | "false" | "" => false,
            other => {
                self.problems.push(format!(
                    "sheet `{}` line {}: {what} `{other}` is neither yes nor no",
                    self.sheet, self.line
                ));
                false
            }
        }
    }

    fn list(&self, col: usize) -> Vec<String> {
        self.text(col)
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }

    fn event(&mut self) -> EventRow {
        EventRow {
            name: self.text(0),
            version: self.number(1, "the version"),
            module: self.text(2),
            object: self.text(3),
            action: self.text(4),
            scope: self.text(5),
            status: self.text(6),
            actor_types: self.list(7),
            description: self.text(8),
            fires_when: self.text(9),
            added_in: self.number(10, "the plan version it was added in"),
            openspec_change: self.text(11),
        }
    }

    fn property(&mut self) -> PropertyRow {
        PropertyRow {
            event: self.text(0),
            version: self.number(1, "the version"),
            property: self.text(2),
            kind: self.text(3),
            list: self.yes_no(4, "List"),
            of: self.text(5),
            required: self.yes_no(6, "Required"),
            classification: self.text(7),
            pattern: self.text(8),
            description: self.text(9),
            example: self.text(10),
        }
    }

    fn record(&mut self) -> RecordRow {
        RecordRow {
            record: self.text(0),
            field: self.text(1),
            kind: self.text(2),
            list: self.yes_no(3, "List"),
            enumeration: self.text(4),
            required: self.yes_no(5, "Required"),
            classification: self.text(6),
            pattern: self.text(7),
            description: self.text(8),
        }
    }

    fn enumeration(&mut self) -> EnumRow {
        EnumRow {
            enumeration: self.text(0),
            value: self.text(1),
            description: self.text(2),
        }
    }

    fn actor(&mut self) -> ActorRow {
        ActorRow {
            kind: self.text(0),
            id_is: self.text(1),
            description: self.text(2),
        }
    }

    fn coverage(&mut self) -> CoverageRow {
        CoverageRow {
            kind: self.text(0),
            name: self.text(1),
            events: self.list(2),
            exemption: self.text(3),
        }
    }

    fn changelog(&mut self) -> ChangelogRow {
        ChangelogRow {
            date: self.text(0),
            plan_version: self.number(1, "the plan version"),
            change: self.text(2),
            pr: self.text(3),
        }
    }
}

/// A cell to write.
enum Cell {
    Text(String),
    Number(f64),
}

fn yes_no(value: bool) -> Cell {
    Cell::Text(if value { "yes" } else { "no" }.to_owned())
}

fn texts(values: &[String]) -> Cell {
    Cell::Text(values.join(", "))
}

/// The rows of each data sheet, in canonical order.
fn rows(plan: &Plan, sheet: &str) -> Vec<Vec<Cell>> {
    match sheet {
        EVENTS => plan.events.iter().map(event_cells).collect(),
        PROPERTIES => plan.properties.iter().map(property_cells).collect(),
        RECORDS => plan.records.iter().map(record_cells).collect(),
        ENUMS => plan
            .enums
            .iter()
            .map(|e| texts_of(&[&e.enumeration, &e.value, &e.description]))
            .collect(),
        ACTORS => plan
            .actors
            .iter()
            .map(|a| texts_of(&[&a.kind, &a.id_is, &a.description]))
            .collect(),
        COVERAGE => plan
            .coverage
            .iter()
            .map(|c| {
                vec![
                    Cell::Text(c.kind.clone()),
                    Cell::Text(c.name.clone()),
                    texts(&c.events),
                    Cell::Text(c.exemption.clone()),
                ]
            })
            .collect(),
        _ => plan
            .changelog
            .iter()
            .map(|c| {
                vec![
                    Cell::Text(c.date.clone()),
                    Cell::Number(f64::from(c.plan_version)),
                    Cell::Text(c.change.clone()),
                    Cell::Text(c.pr.clone()),
                ]
            })
            .collect(),
    }
}

fn texts_of(values: &[&String]) -> Vec<Cell> {
    values
        .iter()
        .map(|value| Cell::Text((*value).clone()))
        .collect()
}

fn event_cells(e: &EventRow) -> Vec<Cell> {
    vec![
        Cell::Text(e.name.clone()),
        Cell::Number(f64::from(e.version)),
        Cell::Text(e.module.clone()),
        Cell::Text(e.object.clone()),
        Cell::Text(e.action.clone()),
        Cell::Text(e.scope.clone()),
        Cell::Text(e.status.clone()),
        texts(&e.actor_types),
        Cell::Text(e.description.clone()),
        Cell::Text(e.fires_when.clone()),
        Cell::Number(f64::from(e.added_in)),
        Cell::Text(e.openspec_change.clone()),
    ]
}

fn property_cells(p: &PropertyRow) -> Vec<Cell> {
    vec![
        Cell::Text(p.event.clone()),
        Cell::Number(f64::from(p.version)),
        Cell::Text(p.property.clone()),
        Cell::Text(p.kind.clone()),
        yes_no(p.list),
        Cell::Text(p.of.clone()),
        yes_no(p.required),
        Cell::Text(p.classification.clone()),
        Cell::Text(p.pattern.clone()),
        Cell::Text(p.description.clone()),
        Cell::Text(p.example.clone()),
    ]
}

fn record_cells(r: &RecordRow) -> Vec<Cell> {
    vec![
        Cell::Text(r.record.clone()),
        Cell::Text(r.field.clone()),
        Cell::Text(r.kind.clone()),
        yes_no(r.list),
        Cell::Text(r.enumeration.clone()),
        yes_no(r.required),
        Cell::Text(r.classification.clone()),
        Cell::Text(r.pattern.clone()),
        Cell::Text(r.description.clone()),
    ]
}

/// How far down each dropdown and filter reaches, so rows added by hand
/// get them too.
const VALIDATED_ROWS: u32 = 2_000;

/// The plan as a workbook, in canonical form (rows sorted, fixed styles,
/// the creation date set to the plan version's date): writing the same plan
/// twice gives the same bytes.
pub fn render(plan: &Plan) -> Result<Vec<u8>> {
    let mut plan = plan.clone();
    plan.sort();
    let mut workbook = Workbook::new();
    let created = creation_date(plan.date())?;
    workbook.set_properties(
        &DocProperties::new()
            .set_title("Campbooks tracking plan")
            .set_subject(format!("Tracking plan version {}", plan.version()))
            .set_author("Campbooks")
            .set_company("Campbooks")
            .set_comment(
                "The tracking plan (ADR 0020). Edit it here or with \
                 `cargo run -p campbooks-tracking-plan -- apply <patch.yaml>`.",
            )
            .set_creation_datetime(&created),
    );

    let header = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x1F_29_37))
        .set_border_bottom(FormatBorder::Thin)
        .set_align(FormatAlign::Top);
    let body = Format::new().set_text_wrap().set_align(FormatAlign::Top);

    readme(workbook.add_worksheet(), &plan, &header, &body)?;
    for (name, columns) in SHEETS {
        let sheet = workbook.add_worksheet();
        sheet.set_name(*name)?;
        write_table(sheet, columns, &rows(&plan, name), &header, &body)?;
    }
    workbook.save_to_buffer().context("writing the workbook")
}

/// The document's creation date: the plan version's date, at midnight UTC.
fn creation_date(date: &str) -> Result<ExcelDateTime> {
    let parts: Vec<&str> = date.split('-').collect();
    let parsed = match parts.as_slice() {
        [y, m, d] => match (y.parse(), m.parse(), d.parse()) {
            (Ok(y), Ok(m), Ok(d)) => Some((y, m, d)),
            _ => None,
        },
        _ => None,
    };
    let (year, month, day) = parsed.unwrap_or((2026, 1, 1));
    ExcelDateTime::from_ymd(year, month, day)
        .or_else(|_| ExcelDateTime::from_ymd(2026, 1, 1))
        .context("the plan's date")
}

fn write_table(
    sheet: &mut Worksheet,
    columns: &[Column],
    rows: &[Vec<Cell>],
    header: &Format,
    body: &Format,
) -> Result<()> {
    for (col, column) in (0_u16..).zip(columns) {
        sheet.write_string_with_format(0, col, column.header, header)?;
        sheet.set_column_width(col, column.width)?;
        let mut validation = match column.dropdown {
            Some(values) => DataValidation::new()
                .allow_list_strings(values)?
                .set_error_title("Not in the list")?
                .set_error_message(format!("Pick one of: {}.", values.join(", ")))?,
            None => DataValidation::new().allow_any_value(),
        };
        validation = validation
            .set_input_title(column.header)?
            .set_input_message(column.help)?;
        sheet.add_data_validation(1, col, VALIDATED_ROWS, col, &validation)?;
    }
    for (row, cells) in (1_u32..).zip(rows) {
        for (col, cell) in (0_u16..).zip(cells) {
            match cell {
                Cell::Text(text) if text.is_empty() => {}
                Cell::Text(text) => {
                    sheet.write_string_with_format(row, col, text, body)?;
                }
                Cell::Number(number) => {
                    sheet.write_number_with_format(row, col, *number, body)?;
                }
            }
        }
    }
    let last_col = u16::try_from(columns.len().saturating_sub(1)).unwrap_or(0);
    let last_row = u32::try_from(rows.len()).unwrap_or(u32::MAX).max(1);
    sheet.autofilter(0, 0, last_row, last_col)?;
    sheet.set_freeze_panes(1, 0)?;
    Ok(())
}

/// The README sheet: what the plan is, the rules, how to change it. The
/// tool rewrites it on every write, so it always shows the plan version.
fn readme(sheet: &mut Worksheet, plan: &Plan, header: &Format, body: &Format) -> Result<()> {
    sheet.set_name(README)?;
    sheet.set_column_width(0, 26)?;
    sheet.set_column_width(1, 110)?;
    let title = Format::new().set_bold().set_font_size(16);
    let label = Format::new().set_bold().set_align(FormatAlign::Top);
    sheet.write_string_with_format(0, 0, "Campbooks tracking plan", &title)?;
    let version = format!("{} ({})", plan.version(), plan.date());
    sheet.write_string_with_format(2, 0, "Plan version", &label)?;
    sheet.write_string_with_format(2, 1, version, body)?;
    let mut row = 3_u32;
    for &(heading, text) in README_ROWS {
        if text.is_empty() {
            row += 1;
            sheet.write_string_with_format(row, 0, heading, header)?;
            sheet.write_string_with_format(row, 1, "", header)?;
        } else {
            sheet.write_string_with_format(row, 0, heading, &label)?;
            sheet.write_string_with_format(row, 1, text, body)?;
        }
        row += 1;
    }
    sheet.set_active(true);
    Ok(())
}

/// The README's rows: `(label, text)`; a row with no text is a section
/// heading.
const README_ROWS: &[(&str, &str)] = &[
    (
        "What this is",
        "Every tracked event Campbooks records: who did what, in which context, and \
               what the records it touched looked like at that moment (ADR 0020). This \
               workbook is the source of truth. The tool generates the typed Rust events \
               and the readable mirror tracking/tracking-plan.json from it; never edit \
               those by hand.",
    ),
    ("Naming", ""),
    (
        "Event names",
        "<module>.<object>_<past verb>, e.g. conversations.conversation_archived. \
               Module, Object and Action repeat its parts. Versions start at 1; a published \
               version never changes meaning: a change that breaks a field is a new \
               version, and only the newest version may be live.",
    ),
    (
        "Other names",
        "Properties, fields and enum values are snake_case. Records and enums are \
               <module>.<name> and belong to the event's module.",
    ),
    ("No personal data", ""),
    (
        "Types",
        "id, record_ref, record (a snapshot from Records), enum, bool, int, decimal, \
               duration_ms, timestamp, date, bytes, currency, country, locale, code (needs a \
               Pattern). List = yes makes a list (never of records). There is no free-text \
               type.",
    ),
    ("id", "A record's own id (a conversation, an inbox)."),
    (
        "pseudonymous",
        "Points at a person or a workspace without naming them (an account id, a \
               workspace id). The warehouse hashes these again with its own key.",
    ),
    (
        "operational",
        "Counts, flags, states, codes, times: says nothing about who.",
    ),
    (
        "personal (rejected)",
        "Names, addresses, phone numbers. Write a field down as personal to show it \
               was considered; the check fails until it is removed.",
    ),
    (
        "content (rejected)",
        "Subjects, bodies, file names, anything someone wrote. Rejected like \
               personal.",
    ),
    ("Adding an event", ""),
    (
        "1. Events",
        "Add a row with status planned, the actor types that may cause it, what it \
               records and when it fires.",
    ),
    (
        "2. Properties",
        "One row per property. Snapshots of the module's own rows go through a \
               record (type record): describe its fields once in Records.",
    ),
    (
        "3. Coverage",
        "Map every operation, job, schedule or handler that emits it; give any that \
               emits nothing an exemption reason instead.",
    ),
    (
        "4. Changelog",
        "Add a row with the next plan version, today's date and the PR.",
    ),
    (
        "5. Generate",
        "Run the tool's generate command, track the event in the transaction of the \
               change (Tracker::track), then set its status to live.",
    ),
    ("The tool", ""),
    (
        "Check",
        "cargo run -p campbooks-tracking-plan -- check (CI runs it): the rules above, \
               the generated files are current, every live event is tracked outside tests, \
               and Coverage names real operations.",
    ),
    (
        "Generate",
        "cargo run -p campbooks-tracking-plan -- generate: writes \
               tracking/tracking-plan.json and every tracking.gen.rs.",
    ),
    (
        "Format",
        "cargo run -p campbooks-tracking-plan -- fmt: rewrites this workbook in \
               canonical form (rows sorted, styles, dropdowns). This sheet is rewritten each \
               time.",
    ),
    (
        "Apply a patch",
        "cargo run -p campbooks-tracking-plan -- apply tracking/patches/<n>-<slug>.yaml: \
               adds or updates rows, then formats and generates. Agents and scripts change \
               the plan this way; people may also edit this workbook directly.",
    ),
    ("More", "docs/tracking.md"),
];

/// Write the canonical workbook to `path`.
pub fn write(path: &Path, plan: &Plan) -> Result<()> {
    let bytes = render(plan)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ChangelogRow;

    fn sample() -> Plan {
        let mut plan = Plan::empty();
        plan.changelog.push(ChangelogRow {
            date: "2026-10-23".into(),
            plan_version: 1,
            change: "First".into(),
            pr: "#1".into(),
        });
        plan.events.push(EventRow {
            name: "things.thing_made".into(),
            version: 1,
            module: "things".into(),
            object: "thing".into(),
            action: "made".into(),
            scope: "workspace".into(),
            status: "planned".into(),
            actor_types: vec!["user".into(), "system".into()],
            description: "A thing was made.".into(),
            fires_when: "Someone makes a thing.".into(),
            added_in: 1,
            openspec_change: "add-things".into(),
        });
        plan.properties.push(PropertyRow {
            event: "things.thing_made".into(),
            version: 1,
            property: "size".into(),
            kind: "int".into(),
            required: true,
            classification: "operational".into(),
            description: "How big.".into(),
            ..PropertyRow::default()
        });
        plan.coverage.push(CoverageRow {
            kind: "operation".into(),
            name: "things.make".into(),
            events: vec!["things.thing_made".into()],
            exemption: String::new(),
        });
        plan
    }

    #[test]
    fn writing_then_reading_gives_the_plan_back() {
        let dir = std::env::temp_dir().join(format!("tracking-plan-rt-{}", std::process::id()));
        let path = dir.join("plan.xlsx");
        let mut plan = sample();
        write(&path, &plan).unwrap();
        let read = read(&path).unwrap().unwrap();
        plan.sort();
        assert_eq!(read, plan);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rendering_is_byte_stable() {
        let plan = sample();
        assert_eq!(render(&plan).unwrap(), render(&plan).unwrap());
    }

    #[test]
    fn dates_and_cells() {
        assert_eq!(serial_to_date(46_318.0), "2026-10-23");
        assert_eq!(cell_name(0, 0), "A1");
        assert_eq!(cell_name(4, 27), "AB5");
        assert_eq!(text(&Data::Float(3.0)), "3");
        assert_eq!(text(&Data::Bool(true)), "yes");
    }

    #[test]
    fn every_dropdown_fits_excels_limit() {
        for (_, columns) in SHEETS {
            for column in *columns {
                if let Some(values) = column.dropdown {
                    assert!(DataValidation::new().allow_list_strings(values).is_ok());
                }
            }
        }
    }
}
