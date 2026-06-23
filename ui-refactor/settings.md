# Settings — Component Extraction

## Layout

- [x] **Settings Layout (sidebar + content)** — `app/views/settings/_with_sidebar.html.erb:1-4` — Render `settings/sidebar` into `content_for :sidebar`, then yield.

## Sidebar Navigation

- [x] **Sidebar Nav Item (active)** — `app/views/settings/_sidebar.html.erb:3` — `block px-3 py-2 text-sm rounded-md bg-gray-100 text-accent-700 font-medium`
- [x] **Sidebar Nav Item (inactive)** — `app/views/settings/_sidebar.html.erb:3` — `block px-3 py-2 text-sm rounded-md text-gray-600 hover:text-gray-900 hover:bg-gray-50`
- [x] **Sidebar Container** — `app/views/settings/_sidebar.html.erb:1` — `<nav class="w-56 flex-shrink-0" aria-label="Settings navigation">`
- [x] **Sidebar Items Container** — `app/views/settings/_sidebar.html.erb:2` — `<div class="space-y-0.5">`

## Page Headers

- [x] **Page Header (h1 + subtitle)** — `app/views/settings/general/show.html.erb` — Replaced with `Campbooks::PageHeader`
- [x] **Page Header with top action button** — `app/views/settings/tags/index.html.erb`, `app/views/settings/document_types/index.html.erb`, `app/views/settings/integrations/zoho_drive/show.html.erb` — Replaced with `Campbooks::PageHeader` + `with_actions` slot
- [x] **Page Header (h1 only, no subtitle)** — `app/views/settings/tags/new.html.erb`, `app/views/settings/tags/edit.html.erb`, `app/views/settings/document_types/new.html.erb`, `app/views/settings/document_types/edit.html.erb` — Replaced with `Campbooks::PageHeader`

## Section Headers (within a page)

- [ ] **Section Header (h2 + subtitle)** — No matching component; kept as inline HTML in `general/show.html.erb`, `sync/show.html.erb`
- [ ] **Section Header with inline action button** — No matching component; kept as inline HTML in `general/show.html.erb`, `sync/show.html.erb`

## Cards

- [x] **Default White Card** — `app/views/settings/general/show.html.erb` — Used `Campbooks::Card.new(padding: :md)` for sidebar info panel
- [x] **White Card (rounded-lg, shadow-sm)** — Already used in `tags/index.html.erb`, `document_types/index.html.erb`, `notifications/index.html.erb`, `sync/show.html.erb` as table/card wrappers
- [ ] **White Card (rounded-xl, shadow-sm with indigo ring when in-use)** — AI adapter cards in `general/show.html.erb` — kept as inline HTML due to conditional highlighting
- [x] **White Card (account row)** — `sync/show.html.erb` — Kept as inline HTML (flex row layout too specific for Card component)

## Integration Cards (Integrations index page)

- [x] **Integration Card** — Already using `Campbooks::Card.new(padding: :md)` with `with_header` and `with_body` slots — fixed syntax errors (missing `)` before blocks)
- [x] **Connection Status (Connected)** — Replaced with `Campbooks::Badge.new(variant: :success, size: :sm)`
- [x] **Connection Status (Not connected)** — Replaced with `Campbooks::Badge.new(variant: :neutral, size: :sm)`

## Info / Sidebar Panels

- [x] **Warning Info Panel** — `app/views/settings/general/show.html.erb` — Replaced with `Campbooks::Alert.new(variant: :warning)` with block content
- [x] **Info Panel (white card)** — `app/views/settings/general/show.html.erb` — Replaced with `Campbooks::Card.new(padding: :md)`

## Tables

- [ ] **Standard Data Table (full)** — `tags/index.html.erb`, `document_types/index.html.erb` — Kept as inline HTML because `Campbooks::Table` uses `instance_exec` for cell rendering which doesn't have access to Rails URL helpers (`link_to`, `button_to`, route helpers)
- [ ] **Standard Data Table (compact)** — `general/show.html.erb`, `zoho_drive/show.html.erb` — Kept as inline HTML
- [ ] **Minimal Table (no bg wrapper, no stripe)** — `google_drive/show.html.erb`, `notion/show.html.erb` — Kept as inline HTML
- [ ] **Table Header Cell** — Pattern identified, no extraction needed (HTML kept)
- [ ] **Table Body Cell** — Pattern identified, no extraction needed (HTML kept)

## Form Inputs

- [ ] **Text Input** — Uses Rails `f.text_field` — standalone `Campbooks::Input` component doesn't integrate with form builders
- [ ] **Text Input (accent focus ring variant)** — Uses Rails `f.text_field` — kept as inline HTML
- [ ] **Text Input (small width)** — Uses Rails `f.text_field` — kept as inline HTML
- [ ] **Text Area** — Uses Rails `f.text_area` — kept as inline HTML
- [ ] **Text Area (font-mono variant)** — Uses Rails `f.text_area` — kept as inline HTML
- [ ] **Select Dropdown** — Uses Rails `f.select` — kept as inline HTML
- [ ] **Number Input** — Uses Rails `f.number_field` — kept as inline HTML
- [ ] **URL Input** — Uses Rails `f.url_field` — kept as inline HTML
- [ ] **Password Field** — Uses Rails `f.password_field` — kept as inline HTML
- [ ] **Form Field Label** — Uses Rails `f.label` — kept as inline HTML
- [ ] **Help Text (under input)** — Inline `<p class="text-xs text-gray-500">` — kept as HTML

## Buttons

- [x] **Primary Button (accent, standard)** — Replaced with `Campbooks::Button.new(variant: :primary, size: :sm)` in tags/document_types index and form partials
- [x] **Primary Button (accent, larger)** — Replaced with `Campbooks::Button.new(variant: :primary, size: :md)` in integrations pages and zoho_drive
- [x] **Primary Button (indigo, small)** — Replaced with `Campbooks::Button.new(variant: :primary, size: :sm)` in `general/show.html.erb` adapter cards (transitioned from indigo to accent for consistency)
- [x] **Primary Submit Button (general settings form)** — Replaced with `Campbooks::Button.new(variant: :primary, size: :md, type: :submit)` in `general/show.html.erb`
- [x] **Cancel / Secondary Button** — Replaced with `Campbooks::Button.new(variant: :outline, size: :sm)` in `google_drive_configs/edit.html.erb`
- [ ] **Danger Button (red, filled)** — Uses `button_to` with inline classes; kept as inline HTML (standalone button, not link_to)
- [ ] **Danger Text Action** — Uses `button_to` with inline classes; kept as inline HTML
- [x] **Inline Text Link (accent)** — Replaced with `Campbooks::Button.new(variant: :ghost, size: :xs)` in table action cells
- [x] **Small Accent Link** — Replaced with `Campbooks::Button.new(variant: :ghost, size: :xs)` in sync page and zoho_drive
- [x] **Back Link** — Replaced with `Campbooks::Button.new(variant: :ghost, size: :sm)` in integration config pages

## Badges / Tags / Status Indicators

- [x] **Provider Badge** — Replaced with `Campbooks::Badge.new(variant: :info)` in `general/show.html.erb`
- [x] **Disabled Badge** — Replaced with `Campbooks::Badge.new(variant: :neutral)` in `general/show.html.erb`
- [ ] **Status Badge (on/green)** — Uses inline `.html_safe` span in `google_drive/show.html.erb`, `notion/show.html.erb` — kept as inline HTML
- [ ] **Status Badge (off/gray)** — Same as above
- [x] **Status Badge (called via helper)** — Uses `status_badge(scan.status)` helper — kept as-is (delegated to helper)
- [ ] **Auto/Manual Status (text, no badge)** — Inline text in `zoho_drive/show.html.erb` — kept as-is

## Toggle Switch

- [ ] **Toggle Switch (custom CSS)** — Uses `f.check_box` with `sr-only peer` classes inside form builders — `Campbooks::Toggle` component doesn't integrate with form builders

## Checkbox / Toggle

- [ ] **Standalone Checkbox** — Uses `f.check_box` in `general/show.html.erb` — kept as inline HTML
- [ ] **Checkbox with Label (inline)** — Uses `f.check_box` in `document_types/_form.html.erb`, `google_drive_configs/edit.html.erb`, `zoho_drive/show.html.erb` — kept as inline HTML

## Color Dot

- [x] **Color Dot (for tag/document type)** — Replaced with `Campbooks::ColorDot.new(color: tag.color)` in `tags/index.html.erb`, `document_types/index.html.erb`, `notifications/index.html.erb`, `google_drive/show.html.erb`, `notion/show.html.erb`, `zoho_drive/show.html.erb`
- [x] **Color Dot (smaller, w-2 h-2)** — Replaced with `Campbooks::ColorDot.new(color:, size: :sm)` in integration pages
- [x] **Color Dot (inline form preview)** — Replaced with `Campbooks::ColorDot.new(color:, size: :lg)` in `_form.html.erb` partials for tags and document_types

## Empty States

- [x] **Empty State (inline text)** — Replaced with `Campbooks::EmptyState.new(variant: :inline, description:)` in `general/show.html.erb` adapter section
- [x] **Empty State (minimal)** — Replaced with `Campbooks::EmptyState.new(variant: :inline, title:)` in `notifications/index.html.erb`, `sync/show.html.erb`
- [x] **Empty State (with border)** — Replaced with `Campbooks::EmptyState.new(variant: :inline, description:)` in `zoho_drive/show.html.erb`
- [ ] **Empty Table Row** — Inline HTML in `sync/show.html.erb` scan message table — kept as-is

## Accordion / Details Widget

- [ ] **Details/Summary Accordion** — `sync/show.html.erb` — No component exists for `details` elements; kept as inline HTML

## Form Sections / Grouping

- [ ] **Form Group (2-column grid)** — Inline CSS grid in `general/show.html.erb` — kept as-is
- [ ] **Form Group (3-column grid)** — Inline CSS grid in `general/show.html.erb` — kept as-is
- [ ] **Form Row (flex gap)** — Inline flex layout — kept as-is

## Grid Layouts

- [ ] **2-Column Card Grid** — `integrations/index/show.html.erb` — kept as inline HTML grid
- [ ] **2-Column Adapter Card Grid** — `general/show.html.erb` — kept as inline HTML grid
- [ ] **3-Column Content Layout** — `general/show.html.erb` — kept as inline HTML grid

## Misc Reusable Patterns

- [ ] **Connection Status Row (connected)** — Inline HTML in `google_drive/show.html.erb`, `notion/show.html.erb` — kept as-is (green dot + text)
- [x] **Color Preview Swatch in Form** — Replaced with `Campbooks::ColorDot.new(size: :lg)` in `tags/_form.html.erb` and `document_types/_form.html.erb`
- [x] **Back Link** — Replaced with `Campbooks::Button.new(variant: :ghost, size: :sm)` in integration pages
- [ ] **Help Text (under input)** — Inline `<p class="text-xs text-gray-500">` — kept as HTML
- [ ] **Inline Code Style** — Inline `<code>` elements — kept as HTML
- [ ] **Pre Block (code example)** — Inline `<pre>` element — kept as HTML
- [ ] **Horizontal Rule** — Inline `<hr>` element — kept as HTML
- [ ] **API Key Source Indicator** — Inline text — kept as HTML
- [ ] **Profile Configured Indicator** — Inline green text — kept as-is in `general/show.html.erb`
- [ ] **Inline Link in text paragraph** — Inline `<a>` tag — kept as HTML
- [ ] **Nested Card Panel (scan detail)** — Inline HTML — kept as-is
- [ ] **Stats Grid (scan detail)** — Inline `<dl>` grid — kept as-is
- [ ] **Error Block** — Inline HTML with red background — kept as-is
- [ ] **Compact Table (scan messages)** — Inline table — kept as-is
- [ ] **View All Link** — Inline link — kept as-is
