# Auth + Onboarding + Notifications + Chat — Component Extraction

## Auth Cards & Layout

- [x] **Auth Page Shell** — `app/views/sessions/new.html.erb:1` — `min-h-[60vh] flex items-center justify-center` wrapping `w-full max-w-md`. Used by every auth page. File:line occurrences: `sessions/new.html.erb:1`, `registrations/new.html.erb:1`, `registrations/password.html.erb:1`, `registrations/verify.html.erb:1`, `passwords/new.html.erb:1`, `passwords/edit.html.erb:1`
- [x] **Auth Card** — `app/views/sessions/new.html.erb:8` — `bg-white rounded-xl shadow-sm border border-gray-200 p-8`. Identical card used at: `registrations/new.html.erb:8`, `registrations/password.html.erb:8`, `registrations/verify.html.erb:8`, `passwords/new.html.erb:9`, `passwords/edit.html.erb:8`
- [x] **Auth Form Spacing** — `app/views/sessions/new.html.erb:9` — `space-y-5` on the `<form>` element. Used identically in every auth form.

## Auth Branding Header

- [x] **Auth Brand Title** — `app/views/sessions/new.html.erb:4` — `text-3xl font-bold text-accent-600`. Variants: `registrations/new.html.erb:4`, `registrations/password.html.erb:4`, `registrations/verify.html.erb:4` all identical; `passwords/new.html.erb:5` uses `text-xl font-semibold text-gray-900` for sub-pages.
- [x] **Auth Header Subtext** — `app/views/sessions/new.html.erb:5` — `text-sm text-gray-600 mt-2`. Occurs on every auth page.
- [x] **Auth Centered Text Block** — `app/views/sessions/new.html.erb:3` — `text-center mb-8`. Contains brand title + subtext.

## Auth Form Labels

- [x] **Form Label** — `app/views/sessions/new.html.erb:11` — `block text-sm font-medium text-gray-700 mb-1.5`. Used on every form label in auth, onboarding, and password pages. Files: `sessions/new.html.erb:11,18`, `registrations/new.html.erb:11,19`, `registrations/password.html.erb:16`, `registrations/verify.html.erb:16`, `passwords/new.html.erb:12`, `passwords/edit.html.erb:11,18`

## Auth Form Inputs

- [x] **Text/Email Input (rounded-lg)** — `app/views/sessions/new.html.erb:14` — `block w-full rounded-lg border-gray-300 shadow-sm text-sm focus:border-accent-500 focus:ring-accent-500`. The standard text/email/password input. Used in: `sessions/new.html.erb:14,21`, `registrations/new.html.erb:15,23`, `registrations/password.html.erb:19`, `passwords/new.html.erb:15`, `passwords/edit.html.erb:14,21`
- [x] **Verification Code Input** — `app/views/registrations/verify.html.erb:19` — `block w-full rounded-lg border-gray-300 shadow-sm text-lg text-center tracking-[0.25em] font-mono focus:border-accent-500 focus:ring-accent-500`. Larger centered monospace input for 6-digit code.
- [x] **Form Input (rounded-md variant)** — `app/views/onboarding/steps/ai_configuration.html.erb:34` — `block w-full rounded-md border-gray-300 shadow-sm text-sm focus:border-accent-500 focus:ring-accent-500`. Used throughout onboarding settings forms where inputs are smaller or in grids. Not used in auth.
- [x] **Form Textarea** — `app/views/onboarding/steps/organization.html.erb:75` — Same rounded-md class set but with `rows:` attribute. Used for multi-line text in onboarding and classification custom entries.
- [x] **Form Number Field** — `app/views/onboarding/steps/ai_configuration.html.erb:64` — Same rounded-md class set. Used for numeric AI configuration fields.

## Auth Buttons

- [x] **Primary Submit Button (full-width auth)** — `app/views/sessions/new.html.erb:26` — `w-full flex justify-center py-2.5 px-4 border border-transparent rounded-lg shadow-sm text-sm font-medium text-white bg-accent-600 hover:bg-accent-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-accent-500 cursor-pointer transition-colors`. Used in: `sessions/new.html.erb:26`, `registrations/new.html.erb:28`, `registrations/password.html.erb:24`, `registrations/verify.html.erb:24`, `passwords/new.html.erb:20`, `passwords/edit.html.erb:26`
- [x] **Primary Submit Button (full-width, no flex-center)** — `app/views/onboarding/steps/classification.html.erb:152` — `w-full py-2.5 px-4 border border-transparent rounded-lg shadow-sm text-sm font-medium text-white bg-accent-600 hover:bg-accent-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-accent-500 cursor-pointer transition-colors`. Same as auth variant but without `flex justify-center`. Used at: `onboarding/steps/classification.html.erb:152`, `onboarding/steps/document_types.html.erb:99`, `onboarding/steps/review.html.erb:159`
- [x] **Primary Submit Button (flex-1 inline with Skip)** — `app/views/onboarding/steps/ai_configuration.html.erb:143` — `flex-1 py-2.5 px-4 border border-transparent rounded-lg shadow-sm text-sm font-medium text-white bg-accent-600 hover:bg-accent-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-accent-500 cursor-pointer transition-colors`. Used next to a Skip button. At: `onboarding/steps/ai_configuration.html.erb:144`, `onboarding/steps/email_accounts.html.erb:55`, `onboarding/steps/organization.html.erb:90`
- [x] **Secondary Outline Button** — `app/views/sessions/new.html.erb:43` — `w-full flex justify-center items-center gap-2 py-2.5 px-4 border border-gray-300 rounded-lg shadow-sm text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-accent-500 cursor-pointer transition-colors`. Used for "Sign in with Zoho" button.
- [x] **Skip Button (outline)** — `app/views/onboarding/steps/ai_configuration.html.erb:146` — `py-2.5 px-4 border border-gray-300 rounded-lg shadow-sm text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-accent-500 cursor-pointer transition-colors`. Used at: `onboarding/steps/ai_configuration.html.erb:146`, `onboarding/steps/email_accounts.html.erb:57`
- [x] **Text Link Button (inline link)** — `app/views/registrations/verify.html.erb:29` — `text-accent-600 hover:text-accent-700 font-medium text-sm bg-transparent border-0 cursor-pointer p-0`. Used for "Resend code" button (a `<button>` styled as link).
- [x] **Muted Link** — `app/views/registrations/verify.html.erb:30` — `text-gray-400 hover:text-gray-600 text-xs font-medium`. Used for "Start over" link.
- [x] **Inline Text Link** — `app/views/sessions/new.html.erb:30` — `text-accent-600 hover:text-accent-700 font-medium`. Used for "Forgot your password?", "Sign in", "Sign up" links. Files: `sessions/new.html.erb:30,34`, `registrations/new.html.erb:33`, `passwords/new.html.erb:25`

## "Or" Divider

- [x] **Or Divider** — `app/views/sessions/new.html.erb:38-41` — `relative my-5` container with `absolute inset-0 flex items-center` line (`border-t border-gray-200`) and `relative flex justify-center text-sm` text (`bg-white px-3 text-gray-600`).

## Progress Indicator (Onboarding)

- [x] **Step Circle** — `app/views/onboarding/_progress_indicator.html.erb:7` — `w-8 h-8 rounded-full flex items-center justify-center text-sm font-semibold flex-shrink-0`. Variants: completed/past uses `bg-accent-600 text-white`; incomplete uses `bg-gray-200 text-gray-500`.
- [x] **Step Connector Line** — `app/views/onboarding/_progress_indicator.html.erb:20` — `flex-1 mx-1 sm:mx-2 h-0.5`. Variants: completed `bg-accent-600`, incomplete `bg-gray-200`.
- [x] **Step Label** — `app/views/onboarding/_progress_indicator.html.erb:14` — `ml-2 text-sm font-medium`. Variants: active/completed `text-accent-700`, inactive `text-gray-400`.
- [x] **Check Icon (completed step)** — `app/views/onboarding/_progress_indicator.html.erb:9` — `w-5 h-5` SVG with checkmark icon inside completed step circle.

## Onboarding Cards & Sections

- [x] **Section Card (p-6)** — `app/views/onboarding/steps/ai_configuration.html.erb:15` — `bg-white rounded-xl shadow-sm border border-gray-200 p-6`. Most common onboarding card. Used at: `onboarding/steps/document_types.html.erb:12,32,57,71`, `onboarding/steps/tags.html.erb:12,32,54,68`, `onboarding/steps/email_accounts.html.erb:10,27`, `onboarding/steps/review.html.erb:10,48,72,104`
- [x] **Section Card (p-8)** — `app/views/onboarding/steps/organization.html.erb:3` — `bg-white rounded-xl shadow-sm border border-gray-200 p-8`. Used on the organization step for a larger card.
- [x] **Section Title (h1/h2/h3)** — `app/views/onboarding/steps/classification.html.erb:4` — `text-lg font-semibold text-gray-900`. Used as section heading in every onboarding step.
- [x] **Section Description** — `app/views/onboarding/steps/classification.html.erb:5` — `text-sm text-gray-500 mt-1`. Used below section titles.
- [x] **Sub-section Title (bold)** — `app/views/onboarding/steps/organization.html.erb:16` — `text-sm font-semibold text-gray-900 mb-1.5`. Used for form field labels on their own.
- [x] **Sub-section Caption** — `app/views/onboarding/steps/ai_configuration.html.erb:100` — `text-[11px] text-gray-400 mt-0.5`. Small gray caption below descriptions.

## Onboarding Info Banners

- [x] **Info Banner (blue)** — `app/views/onboarding/steps/ai_configuration.html.erb:133` — `bg-blue-50 rounded-lg border border-blue-200 p-4`. Icon: `w-4 h-4 text-blue-600 mt-0.5 flex-shrink-0`. Text: `text-sm text-gray-700`.
- [x] **Info Banner (amber "Why this matters")** — `app/views/onboarding/steps/classification.html.erb:140` — `bg-amber-50 rounded-lg border border-amber-200 p-4`. Icon: `w-4 h-4 text-amber-600 mt-0.5 flex-shrink-0`. Inner text uses `text-sm text-gray-700` with `<strong>Why this matters:</strong>`. Used at: `classification.html.erb:140`, `document_types.html.erb:88`, `email_accounts.html.erb:44`, `organization.html.erb:79`, `tags.html.erb:83`
- [x] **Info Banner (green success)** — `app/views/onboarding/steps/review.html.erb:148` — `bg-green-50 rounded-lg border border-green-200 p-4`. Icon: `w-5 h-5 text-green-600 mt-0.5 flex-shrink-0`. Title: `text-sm font-medium text-green-800`. Description: `text-xs text-green-700 mt-0.5`.

## Onboarding Template Selection

- [x] **Template Radio Button Group** — `app/views/onboarding/steps/document_types.html.erb:14` — Wrapper: `flex items-center gap-4 flex-wrap`. Each option: `<label class="flex items-center gap-2 cursor-pointer">` wrapping a radio: `rounded-full border-gray-300 text-accent-600 focus:ring-accent-500` and label: `text-sm text-gray-700`. Used at: `document_types.html.erb:14-28`, `tags.html.erb:14-28`, `classification.html.erb:25-38`

## Checkable Cards

- [x] **Checkable Card (document type, horizontal)** — `app/views/onboarding/steps/classification.html.erb:46` — `<label class="flex items-start gap-3 p-2.5 rounded-lg border border-gray-200 hover:border-gray-300 cursor-pointer transition-colors">`. Wraps checkbox + color dot + title + description. Grid: `grid grid-cols-1 sm:grid-cols-2 gap-2`. Used at: `classification.html.erb:46`, `document_types.html.erb:43`
- [x] **Checkable Card (tag, compact)** — `app/views/onboarding/steps/classification.html.erb:98` — `<label class="flex items-center gap-2 p-2 rounded-lg border border-gray-200 hover:border-gray-300 cursor-pointer transition-colors">`. Grid: `grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-2`.
- [x] **Checkable Card (tag, with title)** — `app/views/onboarding/steps/tags.html.erb:43` — `<label class="flex items-center gap-2.5 p-2.5 rounded-lg border border-gray-200 hover:border-gray-300 cursor-pointer transition-colors">`. Grid: `grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3`.

## Color Dots

- [x] **Color Dot (w-3)** — `app/views/onboarding/steps/classification.html.erb:49` — `w-3 h-3 rounded-full mt-1.5 flex-shrink-0` with inline `style="background-color: ..."`. Used in document type checkable cards.
- [x] **Color Dot (w-2.5)** — `app/views/onboarding/steps/classification.html.erb:101` — `w-2.5 h-2.5 rounded-full flex-shrink-0`. Used in tag checkable cards (compact).
- [x] **Color Dot (review list)** — `app/views/onboarding/steps/review.html.erb:119` — `w-2.5 h-2.5 rounded-full flex-shrink-0`. Used in review step list items.

## Checkboxes

- [x] **Checkbox (rounded)** — `app/views/onboarding/steps/classification.html.erb:48` — `rounded border-gray-300 text-accent-600 focus:ring-accent-500` (on `check_box_tag`). Used everywhere for tag/type selection.

## AI Suggestions

- [x] **AI Suggestions Panel (white card)** — `app/views/onboarding/steps/document_types.html.erb:57` — `bg-white rounded-xl shadow-sm border border-gray-200 p-6`. Used at: `document_types.html.erb:57`, `tags.html.erb:54`
- [x] **AI Suggestions Panel (gray background)** — `app/views/onboarding/steps/classification.html.erb:60` — `p-4 rounded-lg bg-gray-50`. Used in the combined classification step.
- [x] **AI Suggestions Button** — `app/views/onboarding/steps/document_types.html.erb:62` — `px-3 py-1.5 text-xs font-medium text-accent-700 bg-accent-50 rounded-md hover:bg-accent-100 border border-accent-200 cursor-pointer transition-colors`. Variant at `classification.html.erb:65`: `px-3 py-1` (slightly smaller).
- [x] **AI Suggestions Item** — `app/views/onboarding/suggestions/_document_types.html.erb:4` — `<label class="flex items-start gap-3 p-3 rounded-lg border border-accent-200 bg-accent-50 hover:border-accent-300 cursor-pointer transition-colors">`. Color dot + title + description.
- [x] **AI Suggestions Empty / Fallback Text** — `app/views/onboarding/steps/classification.html.erb:68` — `text-xs text-gray-400 italic`. "Click the button above to generate suggestions."

## Detail/Accordion (Custom Entry Toggle)

- [x] **Details/Summary Toggle** — `app/views/onboarding/steps/classification.html.erb:73` — `<details class="group">` with `<summary class="text-xs font-medium text-gray-500 cursor-pointer hover:text-gray-700">`. Used at: `classification.html.erb:73,122`, `document_types.html.erb:73`, `tags.html.erb:122`

## Tab Navigation

- [x] **Tab Bar** — `app/views/onboarding/steps/classification.html.erb:10` — Container: `flex border-b border-gray-200`. Active tab: `flex-1 px-4 py-3 text-sm font-medium text-accent-600 border-b-2 border-accent-600 bg-accent-50/50`. Inactive tab: `flex-1 px-4 py-3 text-sm font-medium text-gray-500 border-b-2 border-transparent hover:text-gray-700 hover:border-gray-300`.

## Service Assignment Table

- [x] **Service Table** — `app/views/onboarding/steps/ai_configuration.html.erb:83` — `bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden`. Table: `min-w-full divide-y divide-gray-200`. Header row: `bg-gray-50` with `<th class="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase">`. Body rows: `divide-y divide-gray-200`.
- [x] **Select Dropdown (small)** — `app/views/onboarding/steps/ai_configuration.html.erb:116` — `block rounded-md border-gray-300 shadow-sm text-xs focus:border-accent-500 focus:ring-accent-500 py-1`. Variant at line 34: `block w-full rounded-md border-gray-300 shadow-sm text-sm ...`.
- [x] **Checkbox Label Row** — `app/views/onboarding/steps/ai_configuration.html.erb:119` — `flex items-center gap-1.5 text-sm`.

## Adapter Card (AI Configuration)

- [x] **Adapter Card** — `app/views/onboarding/steps/ai_configuration.html.erb:15` — `bg-white rounded-xl shadow-sm border border-gray-200 mb-4`. Inner header: `px-5 py-3 border-b border-gray-100 flex items-center justify-between`. Inner body: `px-5 py-4 space-y-3`.
- [x] **Adapter Name Badge** — `app/views/onboarding/steps/ai_configuration.html.erb:19` — `inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium bg-gray-100 text-gray-500`. Used to show provider/model info or "new".
- [x] **Grid Form (2 columns)** — `app/views/onboarding/steps/ai_configuration.html.erb:28` — `grid grid-cols-2 gap-3`. Used for paired fields (provider/model, max tokens/temperature).

## Email Accounts (Onboarding)

- [x] **Connected Account Row** — `app/views/onboarding/steps/email_accounts.html.erb:13` — `flex items-center justify-between p-3 bg-green-50 rounded-lg`. Email icon: `w-5 h-5 text-green-600`. Account name: `text-sm font-medium text-gray-900`. Provider: `text-xs text-gray-500`.
- [x] **Connected Badge** — `app/views/onboarding/steps/email_accounts.html.erb:21` — `text-xs text-green-700 bg-green-100 rounded-full px-2 py-0.5`.
- [x] **Provider Button (branded)** — `app/views/onboarding/steps/email_accounts.html.erb:32` — `inline-flex items-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium text-white transition-colors cursor-pointer border-0`. Variants: Zoho `bg-red-600 hover:bg-red-700`, Google `bg-blue-600 hover:bg-blue-700`, Microsoft `bg-indigo-600 hover:bg-indigo-700`.
- [x] **Provider Button Row** — `app/views/onboarding/steps/email_accounts.html.erb:31` — `flex gap-3 flex-wrap`.

## Organization Form (Onboarding)

- [x] **Organization Header** — `app/views/onboarding/steps/organization.html.erb:4` — `flex items-center gap-3 mb-6`. Icon box: `w-10 h-10 rounded-lg bg-accent-100 flex items-center justify-center`. Icon: `w-5 h-5 text-accent-600`.
- [x] **Country Select** — `app/views/onboarding/steps/organization.html.erb:28` — `block w-full rounded-lg border-gray-300 shadow-sm text-sm focus:border-accent-500 focus:ring-accent-500` with `include_blank: "Select a country..."`.
- [x] **Address Grid (2 columns)** — `app/views/onboarding/steps/organization.html.erb:45` — `grid grid-cols-2 gap-4`.
- [x] **Textarea (large)** — `app/views/onboarding/steps/organization.html.erb:73` — `rows: 6` with standard rounded-lg input classes and placeholder in paragraph form.
- [x] **Helper Text Below Input** — `app/views/onboarding/steps/organization.html.erb:20` — `text-xs text-gray-400 mt-1`. Used to provide hints below form fields.

## Review Step

- [x] **Review Section Card** — `app/views/onboarding/steps/review.html.erb:10` — `bg-white rounded-xl shadow-sm border border-gray-200 p-6`. Header: `flex items-center justify-between mb-4`. Title: `text-sm font-semibold text-gray-900`. Edit link: `text-xs text-accent-600 hover:text-accent-700 font-medium`.
- [x] **Review Definition List** — `app/views/onboarding/steps/review.html.erb:15` — `grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm`. dt: `text-xs text-gray-400`. dd: `text-gray-900 font-medium`.
- [x] **Review Empty Placeholder** — `app/views/onboarding/steps/review.html.erb:67` — `text-sm text-gray-500`. Used for "No email accounts connected" / "None yet" / "Not configured" message.
- [x] **Chip Badge (review adapter)** — `app/views/onboarding/steps/review.html.erb:84` — `inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium bg-indigo-100 text-indigo-800`.
- [x] **Chip Badge (review purpose)** — `app/views/onboarding/steps/review.html.erb:90` — `inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium bg-white text-gray-600 border border-gray-200`.
- [x] **Review Item Row** — `app/views/onboarding/steps/review.html.erb:118` — `flex items-center gap-2 p-1.5 rounded`. Used for listing document types and tags in review.
- [x] **Finish Section** — `app/views/onboarding/steps/review.html.erb:147` — `space-y-4` wrapping the green banner and final submit button.

## Notification Bell & Dropdown

- [x] **Bell Icon Button** — `app/views/notifications/_bell.html.erb:2` — `relative inline-flex items-center text-sm text-gray-500 hover:text-gray-700 focus:outline-none`.
- [x] **Bell Unread Badge** — `app/views/notifications/_bell.html.erb:8` — `absolute -top-1 -right-1 inline-flex items-center justify-center min-w-[16px] h-4 px-0.5 text-[10px] font-bold text-white bg-red-500 rounded-full`. Displays count (capped at "9+").
- [x] **Dropdown Panel** — `app/views/notifications/_bell.html.erb:14` — `hidden absolute right-0 mt-2 w-80 bg-white rounded-lg shadow-lg ring-1 ring-black/5 z-50`.
- [x] **Dropdown Header** — `app/views/notifications/_bell.html.erb:15` — `p-2 border-b border-gray-100 flex items-center justify-between`. Title: `text-xs font-semibold text-gray-500 uppercase`.
- [x] **Mark All Read Link (dropdown)** — `app/views/notifications/_bell.html.erb:19` — `text-xs text-accent-600 hover:text-accent-800`.
- [x] **Dropdown Footer** — `app/views/notifications/_bell.html.erb:27` — `p-2 border-t border-gray-100`. "View all" link: `block text-center text-xs text-accent-600 hover:text-accent-800 font-medium`.

## Notification Dropdown Items

- [x] **Dropdown Item Link** — `app/views/notifications/_dropdown_item.html.erb:1` — `<%= link_to ... class: "block px-3 py-2 hover:bg-gray-50 transition-colors #{notification.read? ? 'opacity-50' : ''}">`. Read items get `opacity-50`.
- [x] **Dropdown Unread Dot** — `app/views/notifications/_dropdown_item.html.erb:6` — `mt-1.5 h-1.5 w-1.5 flex-shrink-0 rounded-full bg-accent-500`.
- [x] **Dropdown Item Title** — `app/views/notifications/_dropdown_item.html.erb:9` — `text-xs font-medium text-gray-900 line-clamp-1`.
- [x] **Dropdown Item Body** — `app/views/notifications/_dropdown_item.html.erb:16` — `text-xs text-gray-500 line-clamp-1`.
- [x] **Dropdown Item Timestamp** — `app/views/notifications/_dropdown_item.html.erb:18` — `text-[10px] text-gray-400 mt-0.5`. Shows `time_ago_in_words`.
- [x] **Dropdown Item Count Badge (small)** — `app/views/notifications/_dropdown_item.html.erb:12` — `inline-flex items-center justify-center min-w-[14px] h-3.5 px-0.5 text-[9px] font-bold text-white bg-accent-500 rounded-full align-middle`. Shows `xN` when count > 1.

## Notification Full Page Index

- [x] **Index Page Header** — `app/views/notifications/index.html.erb:3` — `text-2xl font-bold text-gray-900`. Subtext: `text-sm text-gray-500 mt-1` showing unread count.
- [x] **Index Header Row** — `app/views/notifications/index.html.erb:3` — `flex items-center justify-between mb-6`. Contains title + mark-all-read action.
- [x] **Mark All Read Link (page)** — `app/views/notifications/index.html.erb:13` — `text-sm text-gray-600 hover:text-gray-800 underline`.
- [x] **Index Notification List** — `app/views/notifications/index.html.erb:19` — `bg-white shadow-sm rounded-lg border border-gray-200 divide-y divide-gray-100`.
- [x] **Notification Item (index)** — `app/views/notifications/_notification.html.erb:1` — `<%= link_to ... class: "block px-4 py-3 hover:bg-gray-50 transition-colors #{notification.read? ? 'opacity-60' : ''}">`.
- [x] **Notification Unread Dot (index)** — `app/views/notifications/_notification.html.erb:4` — `mt-1.5 h-2 w-2 flex-shrink-0 rounded-full bg-accent-500`. (Larger than dropdown variant's `h-1.5 w-1.5`)
- [x] **Notification Title (index)** — `app/views/notifications/_notification.html.erb:7` — `text-sm font-medium text-gray-900`.
- [x] **Notification Body (index)** — `app/views/notifications/_notification.html.erb:14` — `text-sm text-gray-500 mt-0.5 line-clamp-2`.
- [x] **Notification Timestamp (index)** — `app/views/notifications/_notification.html.erb:16` — `text-xs text-gray-400 mt-1`.
- [x] **Notification Count Badge (index)** — `app/views/notifications/_notification.html.erb:10` — `inline-flex items-center justify-center min-w-[18px] h-4 px-1 text-[10px] font-bold text-white bg-accent-500 rounded-full align-middle`. (Larger than dropdown badge)
- [x] **Empty Notification State** — `app/views/notifications/index.html.erb:29` — `text-center py-16 bg-white shadow-sm rounded-lg border border-gray-200`. Icon circle: `w-12 h-12 rounded-full bg-gray-100 mx-auto flex items-center justify-center mb-3`. Icon: `w-6 h-6 text-gray-400`. Text: `text-sm text-gray-500`.
- [x] **Dropdown Empty State** — `app/views/notifications/_dropdown_list.html.erb:6` — `px-3 py-6 text-center text-xs text-gray-400`. Text: "No notifications."

## Toast Notification

- [x] **Toast** — `app/views/notifications/_toast.html.erb:1` — `pointer-events-auto w-full max-w-sm overflow-hidden rounded-lg bg-white shadow-lg ring-1 ring-black/5`. Inner: `p-4`. Layout: `flex items-start gap-3`.
- [x] **Toast Title** — `app/views/notifications/_toast.html.erb:5` — `text-sm font-medium text-gray-900`.
- [x] **Toast Body** — `app/views/notifications/_toast.html.erb:12` — `mt-1 text-sm text-gray-500`. `truncate(notification.body, length: 120)`.
- [x] **Toast Timestamp** — `app/views/notifications/_toast.html.erb:14` — `mt-1 text-xs text-gray-400`.
- [x] **Toast Count Badge** — `app/views/notifications/_toast.html.erb:8` — Same as notification index badge (`min-w-[18px] h-4 px-1 text-[10px] font-bold text-white bg-accent-500`).
- [x] **Mark Read Button (toast)** — `app/views/notifications/_toast.html.erb:17` — `flex-shrink-0 rounded-md bg-accent-50 px-2 py-1 text-xs font-medium text-accent-600 hover:bg-accent-100 border-0 cursor-pointer`. Wrapped in `inline-flex` form.

## Notification Preferences

- [x] **Preferences Header** — `app/views/notifications/index.html.erb:41` — `text-lg font-semibold text-gray-900 mb-4`.
- [x] **Preferences Grid** — `app/views/notifications/index.html.erb:43` — `grid grid-cols-1 md:grid-cols-2 gap-6`.
- [x] **Preference Section Card** — `app/views/notifications/index.html.erb:45` — `bg-white shadow-sm rounded-lg border border-gray-200 p-4`.
- [x] **Preference Section Title** — `app/views/notifications/index.html.erb:46` — `text-sm font-semibold text-gray-700 mb-3`.
- [x] **Preference Row** — `app/views/notifications/index.html.erb:50` — `flex items-center justify-between py-2 border-b border-gray-50 last:border-b-0`.
- [x] **Preference Label** — `app/views/notifications/index.html.erb:53` — `text-sm text-gray-700`.
- [x] **Color Dot (preferences)** — `app/views/notifications/index.html.erb:52` — `w-3 h-3 rounded-full` with inline background color. Same as onboarding color dots.
- [x] **Toggle Switch** — `app/views/notifications/index.html.erb:56` — `relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200`. On: `bg-accent-600`. Off: `bg-gray-200`. Knob: `inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200`. On position: `translate-x-5`. Off position: `translate-x-0`.

## Agent Chat — Layout

- [x] **Chat Container** — `app/views/agent_chat/show.html.erb:8` — `flex h-[calc(100vh-12rem)] mx-auto max-w-5xl bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden`.
- [x] **Thread Sidebar** — `app/views/agent_chat/show.html.erb:10` — `w-64 flex-shrink-0 border-r border-gray-200 flex flex-col bg-gray-50/50`.
- [x] **Thread Sidebar Header** — `app/views/agent_chat/show.html.erb:11` — `px-4 py-3 border-b border-gray-200 flex items-center justify-between`. Title: `text-sm font-semibold text-gray-900`.
- [x] **New Chat Button (icon)** — `app/views/agent_chat/show.html.erb:15` — `inline-flex items-center justify-center w-7 h-7 rounded-lg text-gray-500 hover:bg-gray-200 hover:text-gray-700 transition-colors`. SVG: `w-4 h-4` plus icon.
- [x] **Thread List** — `app/views/agent_chat/show.html.erb:20` — `flex-1 overflow-y-auto min-h-0 p-2 space-y-0.5`.
- [x] **Thread Empty State** — `app/views/agent_chat/show.html.erb:25` — `text-xs text-gray-400 text-center py-8`. Text: "No chats yet".

## Agent Chat — Thread Item

- [x] **Thread Item** — `app/views/agent_chat/_thread_item.html.erb:2` — `<%= link_to ... class: "block px-3 py-2.5 rounded-lg text-sm transition-colors #{active ? 'bg-gray-100 text-gray-900 font-medium' : 'text-gray-600 hover:bg-gray-50 hover:text-gray-900'}">`.
- [x] **Thread Item Title** — `app/views/agent_chat/_thread_item.html.erb:5` — Truncated via `<div class="truncate">`.
- [x] **Thread Item Timestamp** — `app/views/agent_chat/_thread_item.html.erb:6` — `text-[11px] text-gray-400 mt-0.5`. Shows `time_ago_in_words`.

## Agent Chat — Header

- [x] **Chat Header** — `app/views/agent_chat/show.html.erb:33` — `px-6 py-4 border-b border-gray-200 flex items-center gap-2 flex-shrink-0`.
- [x] **Scout Avatar (header)** — `app/views/agent_chat/show.html.erb:34` — `w-7 h-7 rounded-full bg-attention-100 text-attention-600 flex items-center justify-center`. Contains star SVG (`w-4 h-4`).
- [x] **Chat Title** — `app/views/agent_chat/show.html.erb:40` — `text-base font-semibold text-gray-900 truncate`. Displays thread title.

## Agent Chat — Messages

- [x] **Messages Container** — `app/views/agent_chat/_messages_panel.html.erb:5` — `flex-1 overflow-y-auto min-h-0`.
- [x] **Message List Divider** — `app/views/agent_chat/_messages_panel.html.erb:7` — `divide-y divide-gray-50`.
- [x] **Message Row** — `app/views/agent_chat/_message.html.erb:3` — `flex items-start gap-3 chat-message px-4 py-3`. AI variant adds `flex-row-reverse`.
- [x] **AI Avatar** — `app/views/agent_chat/_message.html.erb:6` — `w-6 h-6 rounded-full bg-blue-100 text-blue-500 flex items-center justify-center flex-shrink-0`. Contains star SVG (`w-3.5 h-3.5`).
- [x] **User Avatar** — `app/views/agent_chat/_message.html.erb:12` — `w-6 h-6 rounded-full bg-accent-500 text-white flex items-center justify-center text-[10px] font-semibold flex-shrink-0`. Shows first letter of user name.
- [x] **Message Author** — `app/views/agent_chat/_message.html.erb:19` — `text-[12px] font-semibold text-gray-900`.
- [x] **Message Timestamp** — `app/views/agent_chat/_message.html.erb:20` — `text-[10px] text-gray-400`.
- [x] **Message Bubble (AI)** — `app/views/agent_chat/_message.html.erb:22` — `text-[13px] leading-relaxed rounded-lg px-3 py-2 max-w-[90%] agent-message-content text-gray-700 bg-gray-50`. Uses `render_markdown`.
- [x] **Message Bubble (User)** — `app/views/agent_chat/_message.html.erb:23` — `text-[13px] leading-relaxed rounded-lg px-3 py-2 max-w-[90%] agent-message-content text-white bg-accent-500`.
- [x] **AI Message Actions** — `app/views/agent_chat/_message.html.erb:26` — Conditionally renders `agent_chat/_actions` when `ai_suggested_actions.any?`.

## Agent Chat — Suggested Action Pills

- [x] **Action Pill Container** — `app/views/agent_chat/_actions.html.erb:5` — `mt-3 flex items-center gap-2 flex-wrap`.
- [x] **Action Pill (archive/destructive, gray)** — `app/views/agent_chat/_actions.html.erb:15` — `inline-flex items-center gap-1 text-[12px] font-medium text-gray-600 bg-gray-100 hover:bg-gray-200 border border-gray-300 rounded-full px-3 py-1 transition-colors`. Used for archive, tag-remove.
- [x] **Action Pill (tag-add, blue)** — `app/views/agent_chat/_actions.html.erb:33` — `inline-flex items-center gap-1 text-[12px] font-medium text-blue-700 bg-blue-50 hover:bg-blue-100 border border-blue-200 rounded-full px-3 py-1 transition-colors`.
- [x] **Action Pill (reclassify, accent)** — `app/views/agent_chat/_actions.html.erb:43` — `inline-flex items-center gap-1 text-[12px] font-medium text-accent-700 bg-accent-50 hover:bg-accent-100 border border-accent-200 rounded-full px-3 py-1 transition-colors`.
- [x] **Action SVG Icon** — `app/views/agent_chat/_actions.html.erb:16` — `w-3.5 h-3.5` for all action pill icons.

## Agent Chat — Empty State

- [x] **Chat Empty State** — `app/views/agent_chat/_messages_panel.html.erb:16` — `flex items-center justify-center h-full`. Wrapper: `text-center max-w-md px-6`.
- [x] **Chat Empty Icon Circle** — `app/views/agent_chat/_messages_panel.html.erb:18` — `w-12 h-12 mx-auto rounded-full bg-attention-100 text-attention-600 flex items-center justify-center mb-3`. Contains star SVG (`w-6 h-6`).
- [x] **Chat Empty Title** — `app/views/agent_chat/_messages_panel.html.erb:23` — `text-lg font-semibold text-gray-900 mb-1`.
- [x] **Chat Empty Description** — `app/views/agent_chat/_messages_panel.html.erb:24` — `text-sm text-gray-500 leading-relaxed`.
- [x] **Suggestion Pill (chat empty)** — `app/views/agent_chat/_messages_panel.html.erb:25` — Wrapper: `mt-4 flex flex-wrap justify-center gap-2`. Pills: `inline-flex items-center px-3 py-1 rounded-full text-xs font-medium`. Variants: `bg-blue-50 text-blue-700`, `bg-accent-50 text-accent-700`, `bg-gray-50 text-gray-600`.

## Agent Chat — Input Form

- [x] **Chat Form Container** — `app/views/agent_chat/_messages_panel.html.erb:35` — `px-6 py-4 border-t border-gray-200 flex-shrink-0`.
- [x] **Chat Input Wrapper** — `app/views/agent_chat/_form.html.erb:6` — `flex gap-3`.
- [x] **Chat Textarea** — `app/views/agent_chat/_form.html.erb:7` — `flex-1 border border-gray-200 rounded-lg text-[14px] text-gray-700 placeholder-gray-400 focus:ring-1 focus:ring-blue-400 focus:border-blue-400 focus:outline-none px-3 py-2 resize-none`. Note: uses blue focus ring instead of accent.
- [x] **Send Button** — `app/views/agent_chat/_form.html.erb:14` — `self-end px-4 py-2 bg-blue-600 text-white text-[13px] font-medium rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed`. Note: uses blue instead of accent.
- [x] **Chat Input Hint** — `app/views/agent_chat/_form.html.erb:21` — `mt-1 text-[11px] text-gray-400`. "Enter to send, Shift+Enter for new line".

## Agent Chat — Typing Indicator

- [x] **Typing Indicator Row** — `app/views/agent_chat/_typing.html.erb:1` — `flex items-start gap-3 px-4 py-3`. Same structure as message row.
- [x] **Typing Avatar** — `app/views/agent_chat/_typing.html.erb:2` — Same as AI avatar: `w-6 h-6 rounded-full bg-blue-100 text-blue-500 flex items-center justify-center flex-shrink-0 mt-0.5`. Star SVG.
- [x] **Typing Name** — `app/views/agent_chat/_typing.html.erb:9` — `text-[12px] font-semibold text-gray-900`. Text: "Scout".
- [x] **Typing Status** — `app/views/agent_chat/_typing.html.erb:10` — `text-[10px] text-gray-400`. Text: "thinking...".
- [x] **Typing Bouncing Dots** — `app/views/agent_chat/_typing.html.erb:12` — Three `<span>` elements: `w-2 h-2 rounded-full bg-blue-400` with staggered `animation: typingBounce 1.4s ease-in-out` (delays 0s, 0.2s, 0.4s).

## Key Design Token Inconsistencies Noted

- Chat input uses `blue-600/blue-700/blue-400` for send button and textarea focus, while every other component uses `accent-500/accent-600/accent-700`. Potential opportunity to standardize on accent tokens.
- Chat message timestamps use `text-[10px]` while notification timestamps use `text-xs` and `text-[10px]`. Two competing conventions for small text sizing.
- Form inputs have two variants: `rounded-lg` (auth, org step) and `rounded-md` (onboarding settings). Minor but consistent difference to consider standardizing.
- Notification bell badge uses `bg-red-500` while all other count badges use `bg-accent-500`. The bell badge is the only red element in the scope.
