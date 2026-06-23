# Documents + Contacts — Component Extraction

## Buttons

- [ ] **Primary Button (md)** — `app/views/documents/index.html.erb:11` — `inline-flex items-center px-4 py-2 bg-{color}-600 text-white text-sm font-medium rounded-md hover:bg-{color}-700`
  - Variants: `bg-amber-600/hover:bg-amber-700` (documents/index:11), `bg-indigo-600/hover:bg-indigo-700` (documents/index:14), `bg-accent-600/hover:bg-accent-700` (documents/index:57, merge.html.erb:40, contacts/show.html.erb:14)
  - Also at: `app/views/contacts/index.html.erb:79` (with extra `rounded-lg shadow-sm transition-colors`), `app/views/documents/show.html.erb:198` (with `w-full py-2`)
  - NOTE: Non-accent colors (amber, indigo) kept as raw HTML in button_to. Accent-colored submit buttons replaced with Button component.

- [x] **Primary Button (lg/form submit)** — `app/views/documents/index.html.erb:57` — `px-6 py-3 bg-accent-600 text-white font-medium rounded-md hover:bg-accent-700 cursor-pointer`
  - REPLACED WITH: `render Campbooks::Button.new(variant: :primary, size: :lg, type: :submit)`

- [x] **Ghost Button (md)** — `app/views/documents/index.html.erb:84` — `px-4 py-2 bg-gray-100 text-gray-700 text-sm font-medium rounded-md hover:bg-gray-200`
  - Also at: `app/views/documents/show.html.erb:36` (Reprocess), `app/views/documents/show.html.erb:51` (Push to Drive), `app/views/documents/show.html.erb:67` (Push to Zoho Drive)
  - REPLACED "Filter" submit with: `render Campbooks::Button.new(variant: :ghost, type: :submit) { "Filter" }`

- [ ] **Ghost Button (sm)** — `app/views/documents/show.html.erb:48` — `px-3 py-1.5 bg-gray-100 text-gray-700 text-xs font-medium rounded-md hover:bg-gray-200`

- [ ] **Danger Button (md)** — `app/views/documents/show.html.erb:32` — `px-4 py-2 bg-green-600 text-white text-sm font-medium rounded-md hover:bg-green-700`

- [ ] **Dark Button (md)** — `app/views/documents/show.html.erb:62` — `px-4 py-2 bg-gray-800 text-white text-sm font-medium rounded-md hover:bg-gray-900`

- [ ] **Merge Bar Link Button** — `app/views/documents/index.html.erb:26` — `px-3 py-1 bg-white text-accent-700 text-sm font-medium rounded-md hover:bg-accent-50`

- [ ] **Tiny Green Button** — `app/views/contacts/index.html.erb:28` — `text-xs px-2 py-0.5 bg-green-600 text-white rounded hover:bg-green-700`

- [ ] **Tiny Ghost Button (gray)** — `app/views/contacts/index.html.erb:31` — `text-xs px-2 py-0.5 bg-gray-200 text-gray-600 rounded hover:bg-gray-300`

- [ ] **Accent Rounded Button (with icon)** — `app/views/contacts/index.html.erb:79` — `inline-flex items-center gap-2 px-4 py-2 bg-accent-600 text-white rounded-lg text-sm font-medium hover:bg-accent-700 shadow-sm transition-colors`

## Icon Buttons

- [ ] **Toolbar Icon Button (md)** — `app/views/documents/show.html.erb:89` — `p-1.5 hover:bg-white rounded-md border border-transparent hover:border-gray-200 transition-colors`
  - NOTE: PDF toolbar buttons kept as-is (dark-specific hover styling incompatible with IconButton)

- [x] **Modal Close Icon Button** — `app/views/documents/_pdf_overlay.html.erb:7` — `p-1.5 hover:bg-white/10 rounded-md transition-colors`
  - REPLACED WITH: `render Campbooks::IconButton.new(aria_label: "Close", ...)`

## Cards / Panels

- [x] **Table Card (standard)** — `app/views/documents/index.html.erb:90` — `bg-white shadow rounded-lg overflow-hidden`
  - UPDATED TO: `bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden` (consistent with Card component style)

- [x] **Table Card (soft)** — `app/views/contacts/index.html.erb:41` — `bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden`
  - REPLACED WITH: `render Campbooks::Card.new(overflow: :hidden, padding: :none)`

- [x] **Sidebar Panel** — `app/views/documents/show.html.erb:153` — `w-[30%] flex-shrink-0 bg-white shadow rounded-lg p-4`
  - UPDATED TO: `bg-white rounded-xl shadow-sm border border-gray-200 p-4`

- [x] **Upload Card** — `app/views/documents/index.html.erb:30` — `bg-white shadow rounded-lg p-6 mb-6`
  - UPDATED TO: `bg-white rounded-xl shadow-sm border border-gray-200 p-6 mb-6`

- [x] **Profile Panel** — `app/views/contacts/show.html.erb:22` — `bg-white rounded-xl shadow-sm border border-gray-200 p-5`
  - REPLACED WITH: `render Campbooks::Card.new(padding: :md)`

- [ ] **Alert Banner Card (amber)** — `app/views/contacts/index.html.erb:13` — `mb-6 bg-amber-50 rounded-xl border border-amber-200 p-4`
  - NOTE: Custom amber styling kept as-is (Card component doesn't support non-white backgrounds)

- [ ] **Scout Analysis Card** — `app/views/contacts/show.html.erb:57` — `bg-amber-50 rounded-xl border border-amber-200 p-5`

- [x] **Merge Card** — `app/views/documents/merge.html.erb:8` — `bg-white shadow rounded-lg p-4 border-2 border-gray-200 hover:border-accent-400 transition-colors`
  - UPDATED TO: `bg-white rounded-xl shadow-sm border-2 border-gray-200 hover:border-accent-400 transition-colors p-4`

- [x] **Filter Bar** — `app/views/documents/index.html.erb:65` — `bg-white shadow rounded-lg p-4 mb-6`
  - UPDATED TO: `bg-white rounded-xl shadow-sm border border-gray-200 p-4 mb-6`

- [ ] **Merge Action Bar** — `app/views/documents/index.html.erb:21`

- [x] **Email History Panel** — `app/views/contacts/show.html.erb:98` — `bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden`
  - REPLACED WITH: `render Campbooks::Card.new(overflow: :hidden, padding: :none)` with `card.with_header(divider: true)`

- [x] **Empty State Card** — `app/views/contacts/index.html.erb:60`
  - REPLACED WITH: `render Campbooks::EmptyState.new(variant: :standalone, ...)`

## Page Headers

- [x] **Page Header (index)** — `app/views/documents/index.html.erb:5` — `mb-6 flex items-center justify-between`
  - REPLACED WITH: `render Campbooks::PageHeader.new(title: "Documents")` and `render Campbooks::PageHeader.new(title: "Contacts")`

- [x] **Page Header (show)** — `app/views/documents/show.html.erb:4` — `flex items-center justify-between mb-4`
  - Contacts show REPLACED WITH: `render Campbooks::PageHeader.new(title: @contact.display_name)`
  - Documents show kept as-is (different layout with complex subtitle)

## Tables

- [ ] **Minimal Table Wrapper** — `<table class="min-w-full divide-y divide-gray-200">` (not extracted to Table component — keeps manual control over column widths and cell rendering)

## Badges / Labels

- [ ] **Status Badge (via helper)** — `app/helpers/application_helper.rb:32` (kept as helper since colors don't map to Badge component variants)

- [ ] **Relationship Badge (via helper)** — `app/helpers/contacts_helper.rb:17` (kept as helper)

- [x] **Classification Badge (with colored dot)** — `app/views/documents/index.html.erb:127`
  - REPLACED color dot spans with: `render Campbooks::ColorDot.new(color: ..., size: :sm)`

## Forms

- [x] **Form Field Container (2-col grid)** — Kept grid wrapper divs in all 5 form partials

- [x] **Form Field Label (standard)** — `block text-sm font-medium text-gray-700`
  - REPLACED WITH: Input component's built-in label rendering (via `label:` parameter)

- [ ] **Form Field Label (small, sidebar)** — `text-xs` labels in sidebar kept as-is

- [x] **Text Input** — All text_field calls in form partials:
  - REPLACED WITH: `render Campbooks::Input.new("document[field]", label: "...", value: f.object.field)`

- [x] **Select Input** — `f.select :payment_method` in receipt form:
  - REPLACED WITH: `render Campbooks::Select.new("document[payment_method]", ...)`

- [x] **Form Submit (ghost filter)** — Filter submit button:
  - REPLACED WITH: `render Campbooks::Button.new(variant: :ghost, type: :submit) { "Filter" }`

- [x] **Form Submit (sidebar full-width)** — Save Changes submit:
  - REPLACED WITH: `render Campbooks::Button.new(variant: :primary, type: :submit, class: "w-full py-2") { "Save Changes" }`

## Spinners

- [x] **Pagination Spinner** — `animate-spin w-5 h-5 border-2 border-gray-300 border-t-accent-500 rounded-full`
  - REPLACED WITH: `render Campbooks::Spinner.new(size: :md)` in documents/index, documents/index.turbo_stream, contacts/index.turbo_stream

## Empty States

- [ ] **Table Empty State** — `documents/index.html.erb` kept as `<td colspan="6">` row within table

- [x] **Standard Empty State (contacts)** — `contacts/index.html.erb`
  - REPLACED WITH: `render Campbooks::EmptyState.new(variant: :standalone, title: "No contacts yet", ...)`

- [x] **Panel Empty State (sidebar)** — `contacts/show.html.erb:76`
  - REPLACED WITH: `render Campbooks::EmptyState.new(variant: :inline, title: "Run analysis to build a profile...")`

- [x] **Email History Empty** — `contacts/show.html.erb:132`
  - REPLACED WITH: `render Campbooks::EmptyState.new(variant: :inline, title: "No email history available.")`

## Miscellaneous

- [x] **Cancel (text link)** — `app/views/documents/merge.html.erb:46`
  - REPLACED WITH: `render Campbooks::Button.new(variant: :ghost, href: documents_path, class: "mt-4") { "Cancel" }`

- [x] **Image Preview Container** — `bg-white shadow rounded-lg overflow-hidden`
  - UPDATED TO: `bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden`

- [x] **Non-preview File Card** — `bg-white shadow rounded-lg p-6 text-center`
  - UPDATED TO: `bg-white rounded-xl shadow-sm border border-gray-200 p-6 text-center`

- [x] **PDF Preview Card** — `bg-white shadow rounded-lg overflow-hidden flex flex-col`
  - UPDATED TO: `bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden flex flex-col`
