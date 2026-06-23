# Dashboard + Layouts + Shared — Component Extraction

## Page Headers & Titles

- [x] **Page Header Row** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::PageHeader`
- [x] **Page Title (h1)** — `app/views/dashboard/show.html.erb` — handled by `Campbooks::PageHeader`

## Buttons

- [x] **Primary Button (lg)** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::Button.new(variant: :primary)`
- [x] **Primary Button (md)** — `app/views/shared/modals/*.html.erb` — replaced with `Campbooks::Button.new(variant: :primary, size: :md)`
    - Applied in: `_ai_configuration_form`, `_document_types_form`, `_organization_form`, `_tags_form`
- [x] **Secondary Button (outline)** — `app/views/dashboard/show.html.erb` and `app/views/shared/modals/*.html.erb` — replaced with `Campbooks::Button.new(variant: :outline, size: :md)`
    - Applied in all modal forms and dashboard empty state
- [x] **Icon Button (md)** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::IconButton.new(size: :sm)` for month navigation arrows
- [ ] **Icon Button (sm, dismiss)** — `app/views/shared/_setup_banner.html.erb:29` — `text-gray-400 hover:text-gray-600 cursor-pointer`
- [ ] **Text Link (accent)** — `app/views/dashboard/show.html.erb:42` — `text-sm text-accent-600 hover:text-accent-700 font-medium`
    - Also at: `app/views/dashboard/show.html.erb:56`, `app/views/dashboard/show.html.erb:188`, `app/views/dashboard/show.html.erb:198`, `app/views/dashboard/show.html.erb:221`, `app/views/dashboard/show.html.erb:229`
- [ ] **Today Link (xs)** — `app/views/dashboard/show.html.erb:22` — `text-xs text-accent-600 hover:text-accent-700 font-medium`
- [ ] **Ghost Button (sign out)** — `app/views/shared/_topbar.html.erb:27` — `text-sm text-gray-600 hover:text-gray-800`
- [ ] **Banner CTA (button variant)** — `app/views/shared/_setup_banner.html.erb:21` — `font-medium hover:underline whitespace-nowrap cursor-pointer`
    - Also as link variant at `app/views/shared/_setup_banner.html.erb:26`: — `font-medium hover:underline whitespace-nowrap` (no cursor-pointer on `<a>`)

## Modal / Dialog

- [ ] **Modal Dialog** — `app/views/shared/_setup_modal.html.erb:3` — `fixed inset-0 m-auto rounded-2xl shadow-2xl border border-gray-200/80 p-0 w-full max-w-lg max-h-[85vh] overflow-hidden backdrop:bg-black/40`
- [ ] **Modal Header** — `app/views/shared/_setup_modal.html.erb:5` — `flex items-center justify-between px-6 py-4 border-b border-gray-100`
- [ ] **Modal Title** — `app/views/shared/_setup_modal.html.erb:6` — `text-base font-semibold text-gray-900`
- [ ] **Modal Close Button** — `app/views/shared/_setup_modal.html.erb:8` — `p-1.5 rounded-lg text-gray-400 hover:text-gray-600 hover:bg-gray-100 transition-colors cursor-pointer`
- [x] **Modal Footer** — `app/views/shared/modals/*.html.erb` — buttons replaced with `Campbooks::Button` components within the footer div
    - Also at: `_document_types_form`, `_organization_form`, `_tags_form`

## Cards / Panels

- [x] **Featured Metric Card (attention variant)** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::Card.new(padding: :none, hover: true, overflow: :hidden)`
    - Inner: `relative p-6` managed inside block
    - Attention background: preserved as overlay div inside Card
- [x] **Featured Metric Card (standard)** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::Card.new(padding: :lg, hover: true)`
- [x] **Secondary Metric Card** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::Card.new(padding: :xs, hover: true)`
- [x] **Financial Metric Card** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::Card.new(padding: :md)`
- [x] **Breakdown Panel** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::Card.new(padding: :lg)`
- [x] **List Card** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::Card.new(padding: :none)` with `card.with_header(divider: true)` and `card.with_body`
- [x] **List Card Header** — `app/views/dashboard/show.html.erb` — handled by `Card#with_header(divider: true)`

## Metrics & Data Display

- [ ] **Metric Value (hero)** — `app/views/dashboard/show.html.erb:34` — `text-3xl font-bold text-attention-700 tabular-nums`
- [ ] **Metric Value (hero, standard)** — `app/views/dashboard/show.html.erb:53` — `text-3xl font-bold text-gray-900 tabular-nums`
- [ ] **Metric Value (sm)** — `app/views/dashboard/show.html.erb:67` — `text-xl font-bold text-gray-900 tabular-nums`
    - Also at: `app/views/dashboard/show.html.erb:71`, `app/views/dashboard/show.html.erb:76`, `app/views/dashboard/show.html.erb:81`
- [ ] **Metric Label** — `app/views/dashboard/show.html.erb:35` — `text-sm font-medium text-gray-700`
    - Also at: `app/views/dashboard/show.html.erb:55`
- [ ] **Metric Label (xs)** — `app/views/dashboard/show.html.erb:68` — `text-xs text-gray-600`
    - Also at: `app/views/dashboard/show.html.erb:72`, `app/views/dashboard/show.html.erb:77`, `app/views/dashboard/show.html.erb:82`, `app/views/dashboard/show.html.erb:95`, `app/views/dashboard/show.html.erb:106`
- [ ] **Metric Status** — `app/views/dashboard/show.html.erb:37` — `text-xs text-attention-600` / `text-xs text-gray-500`
- [ ] **Panel Title** — `app/views/dashboard/show.html.erb:115` — `text-base font-semibold text-gray-900 mb-4`
    - Also at: `app/views/dashboard/show.html.erb:139`, `app/views/dashboard/show.html.erb:161`, `app/views/dashboard/show.html.erb:187`, `app/views/dashboard/show.html.erb:214`

## Charts / Progress Bars

- [ ] **Progress Bar (track)** — `app/views/dashboard/show.html.erb:125` — `flex-1 h-1.5 bg-gray-100 rounded-full overflow-hidden`
    - Also at: `app/views/dashboard/show.html.erb:147`, `app/views/dashboard/show.html.erb:171`
- [ ] **Progress Bar (fill)** — `app/views/dashboard/show.html.erb:126` — `h-full rounded-full`
    - Also at: `app/views/dashboard/show.html.erb:148`, `app/views/dashboard/show.html.erb:172`
- [x] **Color Dot** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::ColorDot.new(color:, size: :sm)` in Type breakdown and Top Email Tags
- [ ] **Definition List (chart rows)** — `app/views/dashboard/show.html.erb:117` — `space-y-2.5`
    - Also at: `app/views/dashboard/show.html.erb:141`, `app/views/dashboard/show.html.erb:163`
- [ ] **Chart Row** — `app/views/dashboard/show.html.erb:119` — `flex items-center gap-3`
    - Also at: `app/views/dashboard/show.html.erb:144`, `app/views/dashboard/show.html.erb:165`
- [ ] **Chart Row Label** — `app/views/dashboard/show.html.erb:120` — `text-sm text-gray-700 w-32 flex-shrink-0 flex items-center gap-1.5`
    - Also at: `app/views/dashboard/show.html.erb:166`
- [ ] **Chart Row Value** — `app/views/dashboard/show.html.erb:128` — `text-sm font-medium text-gray-900 w-8 text-right tabular-nums`
    - Also at: `app/views/dashboard/show.html.erb:150`, `app/views/dashboard/show.html.erb:174`

## Lists

- [ ] **List Container** — `app/views/dashboard/show.html.erb:190` — `divide-y divide-gray-100`
    - Also at: `app/views/dashboard/show.html.erb:225`
- [ ] **List Row (tight)** — `app/views/dashboard/show.html.erb:192` — `px-6 py-3 flex items-center justify-between hover:bg-gray-50 transition-colors`
- [ ] **List Row (normal)** — `app/views/dashboard/show.html.erb:227` — `px-6 py-4 flex items-center justify-between hover:bg-gray-50 transition-colors`
- [ ] **List Row Link** — `app/views/dashboard/show.html.erb:229` — `text-sm font-medium text-accent-600 hover:text-accent-700 truncate block`

## Avatars

- [x] **Avatar (xs)** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::Avatar.new(name: contact.display_name, size: :md)` in Recent Contacts list

## Icon Containers

- [ ] **Icon Container (md)** — `app/views/dashboard/show.html.erb:90` — `flex-shrink-0 w-9 h-9 rounded-lg bg-red-50 flex items-center justify-center`
    - Also at: `app/views/dashboard/show.html.erb:101` (with `bg-emerald-50` variant)
- [ ] **Icon (sm, inside container)** — `app/views/dashboard/show.html.erb:91` — `w-4 h-4 text-red-500`
    - Also at: `app/views/dashboard/show.html.erb:102` (with `text-emerald-600` variant)

## Badges / Status

- [ ] **Status Badge Container** — `app/views/dashboard/show.html.erb:145` — `w-28 flex-shrink-0`

## Flash Messages

- [x] **Notice Flash** — `app/views/layouts/application.html.erb` and `onboarding.html.erb` — replaced with `Campbooks::Alert.new(variant: :notice, message: notice)`
- [x] **Alert Flash** — `app/views/layouts/application.html.erb` and `onboarding.html.erb` — replaced with `Campbooks::Alert.new(variant: :alert, message: alert)`
- [ ] **Notice Flash (slim, centered)** — `app/views/layouts/email.html.erb:18` — `bg-green-50 p-2` with inner `text-sm text-green-800 text-center`
- [ ] **Alert Flash (slim, centered)** — `app/views/layouts/email.html.erb:21` — `bg-red-50 p-2` with inner `text-sm text-red-800 text-center`

## Empty States

- [x] **Empty State (inline text)** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::EmptyState.new(variant: :inline, title: "No documents this month.")`
- [x] **Empty State (full, with icon)** — `app/views/dashboard/show.html.erb` — replaced with `Campbooks::EmptyState.new(variant: :card, title:, description:)` with `with_icon` and `with_actions` slots

## Navigation

- [ ] **Top Nav Bar** — `app/views/shared/_topbar.html.erb:5` — `bg-white shadow-sm border-b border-gray-200 px-4 sm:px-6 lg:px-8`
- [ ] **Nav Inner Container** — `app/views/shared/_topbar.html.erb:6` — `flex justify-between h-16`
- [ ] **Logo Link** — `app/views/shared/_topbar.html.erb:9` — `text-xl font-bold text-accent-600`
- [ ] **Nav Link Container** — `app/views/shared/_topbar.html.erb:11` — `hidden sm:ml-8 sm:flex sm:space-x-6`
- [ ] **User Name** — `app/views/shared/_topbar.html.erb:26` — `text-sm text-gray-600`
- [x] **Date/Month Navigation** — `app/views/dashboard/show.html.erb` — prev/next arrows replaced with `Campbooks::IconButton.new(size: :sm)` within `PageHeader#with_actions`

## Alert Banners

- [ ] **Setup Banner** — `app/views/shared/_setup_banner.html.erb:8` — dynamic bg/border classes via helper
- [ ] **Banner Row** — `app/views/shared/_setup_banner.html.erb:11` — `flex items-center justify-between gap-4 px-2 sm:px-4 lg:px-6 py-1.5 text-sm`
- [ ] **Banner Divider** — `app/views/shared/_setup_banner.html.erb:11` — `border-t` with dynamic border color
- [ ] **Banner Dot** — `app/views/shared/_setup_banner.html.erb:13` — `w-1.5 h-1.5 rounded-full inline-block flex-shrink-0 mr-1.5`
- [ ] **Banner Dismiss Button** — `app/views/shared/_setup_banner.html.erb` — kept as-is (uses `button_to` with params; `button_to` wraps in a form, making IconButton replace impractical)

## Spinner

- [x] **Spinner (md)** — `app/views/shared/_setup_modal.html.erb` — replaced with `Campbooks::Spinner.new(size: :md)`

## Toasts / Notifications

- [ ] **Toast Container** — `app/views/shared/_topbar.html.erb:2` — `fixed bottom-0 right-0 z-50 flex flex-col-reverse gap-2 p-4 max-w-sm w-full pointer-events-none`

## Layout Wrappers

- [ ] **HTML (full-height, gray bg)** — `app/views/layouts/application.html.erb:2` — `h-full bg-gray-50`
    - Also at: `app/views/layouts/onboarding.html.erb:2`, `app/views/layouts/email.html.erb:2`
- [ ] **Body (full-height)** — `app/views/layouts/application.html.erb:16` — `h-full`
- [ ] **Main Content Wrapper (max)** — `app/views/layouts/application.html.erb:22` (default yield wrapper) — `max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8`
- [ ] **Main Content Wrapper (slim)** — `app/views/layouts/onboarding.html.erb:17` — `max-w-3xl mx-auto px-4 sm:px-6 lg:px-8 py-8`
- [ ] **Sidebar Layout** — `app/views/layouts/application.html.erb:35` — `flex gap-8` with inner `flex-1 min-w-0` content area
- [ ] **Skip-to-content Link** — `app/views/layouts/application.html.erb:17` — `sr-only focus:not-sr-only focus:absolute focus:top-3 focus:left-3 focus:z-50 focus:px-4 focus:py-2 focus:bg-accent-600 focus:text-white focus:rounded-md focus:shadow-lg focus:outline-none`

## Form Elements

- [ ] **Form Container** — `app/views/shared/modals/_ai_configuration_form.html.erb:2` — `space-y-5`
    - Also at: `app/views/shared/modals/_document_types_form.html.erb:2`, `app/views/shared/modals/_organization_form.html.erb:2`, `app/views/shared/modals/_tags_form.html.erb:2`
- [ ] **Form Intro Text** — `app/views/shared/modals/_ai_configuration_form.html.erb:4` — `text-sm text-gray-600`
    - Also at: `app/views/shared/modals/_document_types_form.html.erb:3`, `app/views/shared/modals/_organization_form.html.erb:4`, `app/views/shared/modals/_tags_form.html.erb:3`
- [ ] **Field Label** — `app/views/shared/modals/_ai_configuration_form.html.erb:9` — `block text-sm font-medium text-gray-700 mb-1`
    - Also at: `app/views/shared/modals/_document_types_form.html.erb:7`, `app/views/shared/modals/_organization_form.html.erb:8`, `app/views/shared/modals/_tags_form.html.erb:7`
- [ ] **Field Hint (xs)** — `app/views/shared/modals/_document_types_form.html.erb:8` — `text-xs text-gray-500 mb-1.5`
    - Also at: `app/views/shared/modals/_organization_form.html.erb:16`, `app/views/shared/modals/_tags_form.html.erb:8`
- [ ] **Field Hint (tiny)** — `app/views/shared/modals/_ai_configuration_form.html.erb:13` — `text-[11px] text-gray-400 mt-1`
    - Also at: `app/views/shared/modals/_ai_configuration_form.html.erb:21`, `app/views/shared/modals/_ai_configuration_form.html.erb:30`, `app/views/shared/modals/_ai_configuration_form.html.erb:38`, `app/views/shared/modals/_ai_configuration_form.html.erb:47`
- [ ] **Text Input** — `app/views/shared/modals/_ai_configuration_form.html.erb:11` — `block w-full rounded-lg border-gray-300 shadow-sm focus:border-accent-500 focus:ring-accent-500 sm:text-sm`
    - Also at: `app/views/shared/modals/_ai_configuration_form.html.erb:20`, `app/views/shared/modals/_ai_configuration_form.html.erb:28`, `app/views/shared/modals/_document_types_form.html.erb:11`, `app/views/shared/modals/_organization_form.html.erb:10`, `app/views/shared/modals/_tags_form.html.erb:11` and many more
- [ ] **Select Input** — `app/views/shared/modals/_ai_configuration_form.html.erb:20` — `block w-full rounded-lg border-gray-300 shadow-sm focus:border-accent-500 focus:ring-accent-500 sm:text-sm`
- [ ] **Textarea** — `app/views/shared/modals/_document_types_form.html.erb:20` — `block w-full rounded-lg border-gray-300 shadow-sm focus:border-accent-500 focus:ring-accent-500 sm:text-sm`
    - Also at: `app/views/shared/modals/_organization_form.html.erb:18`, `app/views/shared/modals/_tags_form.html.erb:20`
- [ ] **Color Input** — `app/views/shared/modals/_document_types_form.html.erb:27` — `block h-10 w-16 rounded-lg border-gray-300 shadow-sm focus:border-accent-500 focus:ring-accent-500 cursor-pointer`
    - Also at: `app/views/shared/modals/_tags_form.html.erb:27`
- [ ] **Password Field** — `app/views/shared/modals/_ai_configuration_form.html.erb:36` — (same class as Text Input)
- [ ] **Details/Summary (advanced section)** — `app/views/shared/modals/_ai_configuration_form.html.erb:41` — `text-xs font-medium text-gray-500 hover:text-gray-700 cursor-pointer select-none`

## Layout Grids

- [ ] **Grid 2-col (lg)** — `app/views/dashboard/show.html.erb:28` — `grid grid-cols-1 lg:grid-cols-2 gap-5 mb-5`
- [ ] **Grid 4-col (responsive)** — `app/views/dashboard/show.html.erb:65` — `grid grid-cols-2 sm:grid-cols-4 gap-4 mb-8`
- [ ] **Grid 2-col** — `app/views/dashboard/show.html.erb:87` — `grid grid-cols-2 gap-4 mb-8`
- [ ] **Grid 2-col (lg, wider gaps)** — `app/views/dashboard/show.html.erb:113` — `grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8`
- [ ] **Form Grid 2-col** — `app/views/shared/modals/_organization_form.html.erb:22` — `grid grid-cols-2 gap-3`
    - Also at: `app/views/shared/modals/_organization_form.html.erb:37`
- [ ] **Form Grid 5-col (adapter)** — `app/views/shared/modals/_ai_configuration_form.html.erb:7` — `grid grid-cols-5 gap-3`

## Misc Utilities

- [ ] **Section Spacer** — `app/views/dashboard/show.html.erb:64` — `mb-8`
- [ ] **Dot Separator** — `app/views/dashboard/show.html.erb:232` — `<span aria-hidden="true">·</span>`
