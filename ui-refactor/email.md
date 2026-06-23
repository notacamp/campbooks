# Email — Component Extraction

## Icon Buttons
- [x] **Icon Button (xsm, w-4 h-4)** — `app/views/email_messages/show.html.erb:272` — `w-5 h-5 flex items-center justify-center rounded text-gray-400 hover:text-gray-600`
  also at: `app/views/email_messages/show.html.erb:273`
- [x] **Icon Button (sm, w-5 h-5)** — `app/views/email_messages/index.html.erb:8` — `w-5 h-5 flex items-center justify-center rounded text-gray-400 hover:text-gray-600 hover:bg-gray-100 transition-colors`
  also at: `app/views/email_messages/index.html.erb:38`, `app/views/email_messages/show.html.erb:7`, `app/views/email_messages/show.html.erb:55`, `app/views/email_messages/show.html.erb:217`, `app/views/email_messages/show.html.erb:272`
- [x] **Icon Button (md, w-6 h-6)** — `app/views/email_messages/index.html.erb:38` — `w-6 h-6 flex items-center justify-center rounded text-gray-400 hover:text-gray-600 hover:bg-gray-100 transition-colors`
  also at: `app/views/email_messages/show.html.erb:32`
- [x] **Icon Button (add-tag, w-6 h-6 rounded)** — `app/views/email_messages/_tags.html.erb:23` — `inline-flex items-center justify-center text-sm text-gray-400 hover:text-accent-600 font-medium w-6 h-6 rounded hover:bg-gray-100 transition-colors`
  also at: `app/views/email_messages/_zoho_labels.html.erb:23` (same classes)
- [x] **Icon Button (minus/remove, inline)** — `app/views/email_messages/_tags.html.erb:17` — `hover:opacity-60 leading-none text-[12px] ml-0.5`
  also at: `app/views/email_messages/_zoho_labels.html.erb:17`

## Primary Buttons
- [x] **Primary Button (accent)** — `app/views/email_accounts/index.html.erb:3` — `inline-flex items-center rounded-md bg-accent-600 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-accent-500`
  also at: `app/views/email_scans/index.html.erb:3`, `app/views/email_sync/show.html.erb:93`
- [x] **Primary Button (full-width)** — `app/views/email_accounts/show.html.erb:101` — `w-full px-3 py-1.5 bg-accent-600 text-white text-xs font-semibold rounded-lg hover:bg-accent-700 transition-colors cursor-pointer`
- [x] **Primary Button (provider, red)** — `app/views/email_accounts/new.html.erb:11` — `inline-flex items-center rounded-md bg-red-600 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-red-500`
- [x] **Primary Button (provider, blue)** — `app/views/email_accounts/new.html.erb:15` — `inline-flex items-center rounded-md bg-blue-600 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-blue-500`
- [x] **Modal Done Button** — `app/views/email_messages/show.html.erb:308` — `px-4 py-1.5 bg-accent-600 text-white text-xs font-medium rounded-lg hover:bg-accent-700 transition-colors`

## Link-style Buttons
- [x] **Link Button (accent, xs)** — `app/views/email_accounts/index.html.erb:45` — `text-xs font-medium text-accent-600 hover:text-accent-700`
  also at: `app/views/email_accounts/show.html.erb:50`, `app/views/email_sync/show.html.erb:38`
- [x] **Link Button (danger, xs)** — `app/views/email_accounts/index.html.erb:47` — `text-xs text-red-600 hover:text-red-800`
  also at: `app/views/email_accounts/show.html.erb:55` — `text-xs text-red-500 hover:text-red-700 ml-2`
- [x] **Link Button (sm, accent)** — `app/views/email_sync/show.html.erb:71` — `text-sm text-accent-600 hover:text-accent-700 font-medium`
  also at: `app/views/email_sync/show.html.erb:94`

## Contextual Action Pills
- [x] **Action Pill (add_tag)** — `app/views/email_comments/_actions.html.erb:12` — `inline-flex items-center gap-1 text-[11px] font-medium text-blue-700 bg-blue-50 hover:bg-blue-100 border border-blue-200 rounded-full px-2.5 py-0.5 transition-colors`
- [x] **Action Pill (remove_tag/archive)** — `app/views/email_comments/_actions.html.erb:21` — `inline-flex items-center gap-1 text-[11px] font-medium text-gray-600 bg-gray-100 hover:bg-gray-200 border border-gray-300 rounded-full px-2.5 py-0.5 transition-colors`
  also at: `app/views/email_comments/_actions.html.erb:39`
- [x] **Action Pill (draft_reply)** — `app/views/email_comments/_actions.html.erb:30` — `inline-flex items-center gap-1 text-[11px] font-medium text-accent-700 bg-accent-50 hover:bg-accent-100 border border-accent-200 rounded-full px-2.5 py-0.5 transition-colors`
- [x] **Question Option Pill** — `app/views/email_comments/_draft_questions.html.erb:23` — `text-[11px] text-gray-600 bg-white hover:bg-gray-100 border border-gray-300 rounded-full px-2.5 py-0.5 cursor-pointer transition-colors font-medium`
  also at: `app/views/email_comments/_questions.html.erb:14`

## Tag/Label Pills
- [x] **Tag Pill (non-interactive)** — `app/views/email_messages/_tags.html.erb:11` — `text-[11px] rounded px-2 py-0.5 font-medium inline-flex items-center gap-1 select-none` (color via inline style)
  also at: `app/views/email_messages/_zoho_labels.html.erb:11`
- [x] **Tag Pill (inline thread list)** — `app/views/email_messages/_thread_tags.html.erb:2` — `flex items-center gap-1 flex-shrink-0` with inline `font-size:9px;color`
  also at: `app/views/email_messages/_thread_zoho_labels.html.erb:2`

## Permission/Status Badges
- [x] **Permission Badge (md, Read/active)** — `app/views/email_accounts/index.html.erb:39` — `inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-green-50 text-green-700`
- [x] **Permission Badge (md, Send/active)** — `app/views/email_accounts/index.html.erb:42` — `inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-blue-50 text-blue-700`
- [x] **Permission Badge (md, Owner)** — `app/views/email_accounts/index.html.erb:32` — `inline-flex items-center ml-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-amber-50 text-amber-700`
  also at: `app/views/email_accounts/index.html.erb:69`, `app/views/email_accounts/show.html.erb:66`
- [x] **Permission Badge (sm, R/S/M)** — `app/views/email_accounts/index.html.erb:73` — `inline-flex px-1 py-0.5 rounded text-[9px] font-medium` (color conditional: `bg-green-50 text-green-600` / `bg-blue-50 text-blue-600` / `bg-purple-50 text-purple-600` / `bg-gray-100 text-gray-400`)
  also at: `app/views/email_accounts/show.html.erb:62-64`
- [x] **Status Badge (md, Active)** — `app/views/email_accounts/show.html.erb:12` — `inline-flex px-2 py-0.5 rounded text-xs font-medium bg-green-50 text-green-700`
- [x] **Status Badge (md, Inactive)** — `app/views/email_accounts/show.html.erb:14` — `inline-flex px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-500`
- [x] **Sender Badge (Sent)** — `app/views/email_messages/show.html.erb:97` — `text-[10px] font-medium text-blue-500 bg-blue-50 rounded px-1 py-0.5 uppercase`
  also at: `app/views/email_messages/show.html.erb:178` (uses `bg-blue-100`)
- [x] **Sender Badge (Received)** — `app/views/email_messages/show.html.erb:100` — `text-[10px] font-medium text-gray-500 bg-gray-100 rounded px-1 py-0.5 uppercase`
  also at: `app/views/email_messages/show.html.erb:181`
- [x] **Selected Indicator** — `app/views/email_messages/show.html.erb:188` — `text-[9px] text-accent-500 font-medium bg-accent-50 rounded px-1.5 py-0.5`
- [x] **AI Processed Badge** — `app/views/email_messages/show.html.erb:108` — `text-[10px] font-semibold text-accent-600 bg-accent-100 rounded px-1.5 py-0.5`
- [x] **Saved to Zoho Badge** — `app/views/email_comments/_draft_saved.html.erb:2` — `text-[11px] font-medium text-emerald-600 bg-emerald-50 rounded-lg px-2.5 py-1`

## Avatar Circles
- [x] **AI Avatar (md, 6)** — `app/views/email_comments/_comment.html.erb:6` — `w-6 h-6 rounded-full bg-blue-100 text-blue-500 flex items-center justify-center flex-shrink-0`
  also at: `app/views/email_comments/_draft.html.erb:7`, `app/views/email_comments/_draft_questions.html.erb:3`, `app/views/email_comments/_typing.html.erb:2`
- [x] **User Avatar (md, 6)** — `app/views/email_comments/_comment.html.erb:12` — `w-6 h-6 rounded-full bg-accent-500 text-white flex items-center justify-center text-[10px] font-semibold flex-shrink-0`
- [ ] **Thread Avatar (lg, 8)** — `app/views/email_messages/_thread_list.html.erb:33` — `w-8 h-8 rounded-full flex items-center justify-center` (conditional bg, box-shadow for account color, `font-size:12px;font-weight:600`)
- [ ] **Show Avatar (md, 7)** — `app/views/email_messages/show.html.erb:92` — `w-7 h-7 rounded-full flex items-center justify-center text-xs font-semibold flex-shrink-0` (conditional bg)
- [ ] **Message Avatar (sm, 6)** — `app/views/email_messages/show.html.erb:172` — `w-6 h-6 rounded-full flex items-center justify-center text-[10px] font-semibold flex-shrink-0` (conditional bg)

## Direction Indicator (on thread avatar)
- [ ] **Sent Arrow (up)** — `app/views/email_messages/_thread_list.html.erb:38` — small badge overlay: `w-3.5 h-3.5 rounded-full bg-white border border-gray-200 flex items-center justify-center` with SVG arrow
- [ ] **Received Arrow (down)** — `app/views/email_messages/_thread_list.html.erb:40` — same container with SVG down arrow

## Unread Indicator
- [ ] **Unread Dot** — `app/views/email_messages/_thread_list.html.erb:44` — `absolute -top-0.5 -right-0.5 w-2.5 h-2.5 rounded-full bg-blue-500 border-2 border-white`

## Chat Message Bubbles
- [x] **AI Chat Bubble** — `app/views/email_comments/_comment.html.erb:22-23` — `text-[13px] leading-relaxed rounded-lg px-3 py-2 max-w-[90%] agent-message-content text-gray-700 bg-gray-50`
- [x] **User Chat Bubble** — `app/views/email_comments/_comment.html.erb:23` — `text-[13px] leading-relaxed rounded-lg px-3 py-2 max-w-[90%] text-white bg-accent-500`
- [x] **Chat Message Container** — `app/views/email_comments/_comment.html.erb:3` — `flex items-start gap-3 chat-message`
  (AI: normal, User/Human: adds `flex-row-reverse`)

## Thread List Items
- [ ] **Thread List Row** — `app/views/email_messages/_thread_list.html.erb:17` — `flex items-center transition-colors group` with `active ? 'bg-accent-50/50' : 'hover:bg-gray-50/70'`
- [ ] **Thread Subject** — `app/views/email_messages/_thread_list.html.erb:49` — style attribute `font-size:11px;line-height:1.3`, with `'font-bold' if unread`
- [ ] **Thread Date** — `app/views/email_messages/_thread_list.html.erb:50` — style attribute `font-size:10px;line-height:1.3` with conditional `'font-medium text-gray-600' if unread`
- [ ] **Thread Sender Name** — `app/views/email_messages/_thread_list.html.erb:58` — style `font-size:10px`, text-gray-500 truncate
- [ ] **Thread "To:" label (sent)** — `app/views/email_messages/_thread_list.html.erb:54` — `text-blue-500 flex-shrink-0` with `font-size:9px`
- [ ] **Thread Message Count** — `app/views/email_messages/_thread_list.html.erb:61` — `text-gray-400 flex-shrink-0` with `font-size:9px`
- [ ] **Thread Link Container** — `app/views/email_messages/_thread_list.html.erb:29` — `flex items-start gap-3 px-2 py-2.5 flex-1 min-w-0` with `active ? 'text-accent-700' : ''`

## Folder Sidebar Items
- [x] **Folder Item (expanded)** — `app/views/email_messages/index.html.erb:18` — `flex items-center gap-2.5 px-2 py-1.5 rounded-md transition-colors mb-0.5 group` with `active ? 'bg-accent-50 text-accent-700 font-medium' : 'text-gray-600 hover:bg-gray-50'`
  also at: `app/views/email_messages/show.html.erb:17`
- [x] **Folder Icon Container** — `app/views/email_messages/index.html.erb:20` — `w-5 h-5 flex items-center justify-center flex-shrink-0`
  also at: `app/views/email_messages/show.html.erb:19`
- [x] **Folder Name** — `app/views/email_messages/index.html.erb:23` — `flex-1 truncate text-xs`
- [x] **Folder Item (collapsed)** — `app/views/email_messages/index.html.erb:45` — `w-8 h-7 flex items-center justify-center rounded-md transition-colors relative` with `active ? 'bg-accent-50 text-accent-600' : 'text-gray-400 hover:text-gray-600 hover:bg-gray-50'`
  also at: `app/views/email_messages/show.html.erb:39`

## Folder Count Badges
- [x] **Folder Count (expanded)** — `app/views/email_messages/index.html.erb:24` — `inline-flex items-center justify-center min-w-[13px] h-[13px] rounded-full text-[6px] font-semibold tabular-nums flex-shrink-0 px-[3px] leading-none` with `active ? 'bg-accent-100 text-accent-700' : 'bg-gray-100 text-gray-500'`
  also at: `app/views/email_messages/show.html.erb:23`
- [x] **Folder Count (collapsed)** — `app/views/email_messages/index.html.erb:50` — `absolute -top-0.5 -right-0.5 min-w-[11px] h-[11px] rounded-full text-[6px] font-bold tabular-nums flex items-center justify-center leading-none` with `active ? 'bg-accent-500 text-white' : 'bg-gray-400 text-white'` and inline padding
  also at: `app/views/email_messages/show.html.erb:44`

## Section/Page Headers
- [x] **Page Header (h1)** — `app/views/email_accounts/index.html.erb:2` — `text-2xl font-bold text-gray-900`
- [x] **Page Header Bar** — `app/views/email_accounts/index.html.erb:1` — `mb-6 flex items-center justify-between`
- [x] **Section Header (uppercase)** — `app/views/email_messages/_thread_list.html.erb:2` — `text-[10px] font-semibold text-gray-400 uppercase tracking-wider px-3 py-2 bg-gray-50/50 sticky top-0 border-b border-gray-100`
- [x] **Folders Header (uppercase)** — `app/views/email_messages/index.html.erb:6` — `text-[10px] font-semibold text-gray-400 uppercase tracking-wider`
  also at: `app/views/email_messages/show.html.erb:6`
- [x] **Inbox Section Header** — `app/views/email_messages/show.html.erb:53` — `text-[10px] font-semibold text-gray-400 uppercase tracking-wider`
- [x] **Section Title (sm)** — `app/views/email_messages/show.html.erb:270` — `text-sm font-semibold text-gray-900`
  also at: `app/views/email_accounts/show.html.erb:24,80`
- [ ] **Breadcrumb** — `app/views/email_scans/show.html.erb:3` — `flex items-center space-x-2` with separator span `/`

## Cards
- [x] **Card (white, shadow)** — `app/views/email_accounts/index.html.erb:7` — `bg-white shadow rounded-lg p-8 text-center`
- [x] **Card (white, overflow)** — `app/views/email_scans/index.html.erb:6` — `bg-white shadow rounded-lg overflow-hidden`
- [x] **Card (white, p-5)** — `app/views/email_accounts/show.html.erb:79` — `bg-white shadow rounded-lg p-5`
- [x] **Card (stat box)** — `app/views/email_sync/show.html.erb:8` — `bg-white overflow-hidden shadow rounded-lg p-5`
- [x] **Card (link/quick)** — `app/views/email_sync/show.html.erb:153` — `bg-white shadow rounded-lg p-4 hover:shadow-md transition-shadow`

## Form Inputs
- [x] **Text Input (xs)** — `app/views/email_accounts/show.html.erb:84` — `w-full text-xs border-gray-200 rounded-md focus:ring-1 focus:ring-accent-400 focus:border-accent-400`
- [ ] **Text Input (dropdown search)** — `app/views/email_messages/_tags.html.erb:30` — `w-full border-0 border-b border-gray-200 px-3 py-2 text-xs focus:ring-0 focus:outline-none sticky top-0 bg-white rounded-t-lg`
  also at: `app/views/email_messages/_zoho_labels.html.erb:30`
- [x] **Text Area (chat)** — `app/views/email_comments/_form.html.erb:9` — `w-full border border-gray-200 rounded-lg text-[13px] text-gray-700 placeholder-gray-400 focus:ring-1 focus:ring-blue-400 focus:border-blue-400 focus:outline-none px-3 py-2 resize-none`
- [x] **Text Area (draft body)** — `app/views/email_comments/_draft.html.erb:23` — `w-full text-[13px] text-gray-700 leading-relaxed bg-white border border-gray-200 rounded-lg p-3 mb-2 focus:ring-1 focus:ring-blue-400 focus:border-blue-400 focus:outline-none resize-y`
- [x] **Text Input (question answer)** — `app/views/email_comments/_draft_questions.html.erb:33` — `flex-1 text-[12px] border border-gray-300 rounded-lg px-2.5 py-1 focus:ring-1 focus:ring-blue-400 focus:border-blue-400 focus:outline-none`
- [x] **Checkbox (default)** — `app/views/email_accounts/show.html.erb:39` — `w-3 h-3 rounded border-gray-300 text-accent-600 focus:ring-accent-500`
  also at: `app/views/email_accounts/show.html.erb:89,93,97`
- [ ] **Checkbox (inbox/settings)** — `app/views/email_messages/show.html.erb:282` — `w-3.5 h-3.5 rounded border-gray-300 text-accent-600 focus:ring-accent-500`
  also at: `app/views/email_messages/index.html.erb:22,61`
- [x] **Inline Submit Button (xs, accent)** — `app/views/email_accounts/show.html.erb:50` — `text-xs font-medium text-accent-600 hover:text-accent-700 cursor-pointer bg-transparent border-0`

## Chat / Draft Action Buttons
- [x] **Draft Button (blue, primary)** — `app/views/email_comments/_draft.html.erb:27` — `text-[11px] font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg px-3 py-1 transition-colors cursor-pointer`
  also at: `app/views/email_comments/_draft_questions.html.erb:34`
- [x] **Draft Button (emerald, send)** — `app/views/email_comments/_draft.html.erb:31` — `text-[11px] font-medium text-white bg-emerald-600 hover:bg-emerald-700 rounded-lg px-3 py-1 transition-colors`
  also at: `app/views/email_comments/_draft_saved.html.erb:5`
- [x] **Draft Button (discard)** — `app/views/email_comments/_draft.html.erb:35` — `text-[11px] font-medium text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded-lg px-3 py-1 transition-colors`
  also at: `app/views/email_comments/_draft_saved.html.erb:9`
- [x] **Chat Send Button** — `app/views/email_comments/_form.html.erb:13-14` — `px-3 py-1.5 bg-blue-600 text-white text-[12px] font-medium rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed`
- [x] **Send Question Answer Button** — `app/views/email_comments/_draft_questions.html.erb:34` — `text-[11px] font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg px-3 py-1 cursor-pointer transition-colors`

## Account Color Dots
- [x] **Color Dot (4)** — `app/views/email_accounts/index.html.erb:18` — `w-4 h-4 rounded-full flex-shrink-0` (inline style bg)
- [x] **Color Dot (5)** — `app/views/email_accounts/show.html.erb:9` — `w-5 h-5 rounded-full flex-shrink-0` (inline style bg)
- [x] **Color Dot (2.5)** — `app/views/email_messages/show.html.erb:283` — `w-2.5 h-2.5 rounded-full flex-shrink-0` (inline style bg)
- [x] **Color Dot (1.5, thread)** — `app/views/email_messages/_thread_tags.html.erb:3` — `w-1.5 h-1.5 rounded-full flex-shrink-0` (inline style bg)
  also at: `app/views/email_messages/_thread_zoho_labels.html.erb:3`, `app/views/email_messages/_thread_list.html.erb:71`

## Document Type Badges
- [ ] **Doc Type Badge (inline)** — `app/views/email_messages/show.html.erb:143` — `inline-flex items-center gap-1 rounded px-2 py-0.5 text-[11px] font-medium flex-shrink-0` (inline document_type_badge_style)
- [x] **Doc Type Dot** — `app/views/email_messages/show.html.erb:144` — `w-1.5 h-1.5 rounded-full flex-shrink-0` (inline bg)
- [ ] **Doc Type Label (thread list)** — `app/views/email_messages/_thread_list.html.erb:70` — `flex items-center gap-1 flex-shrink-0` with inline `font-size:9px;color`

## Attachment Chips
- [ ] **Document Attachment Chip** — `app/views/email_messages/show.html.erb:141` — `inline-flex items-center gap-1.5 text-[12px] text-gray-600 bg-white border border-gray-300 rounded-lg px-2.5 py-1 hover:bg-gray-50 hover:border-gray-400 transition-colors max-w-full`
- [ ] **Raw File Attachment Chip** — `app/views/email_messages/show.html.erb:155` — `inline-flex items-center gap-1.5 text-[12px] text-gray-600 bg-gray-100 border border-gray-300 rounded-lg px-2.5 py-1 hover:bg-gray-200 hover:border-gray-400 transition-colors max-w-full`
- [ ] **Attachment Count Indicator (thread list)** — `app/views/email_messages/_thread_list.html.erb:64` — `w-auto h-4 rounded-full bg-gray-200 flex items-center justify-center flex-shrink-0 px-1 gap-0.5` with inline font-size 7-8px

## Account Pill (show header)
- [ ] **Account Pill** — `app/views/email_messages/show.html.erb:81` — `flex-shrink-0 inline-flex items-center gap-1.5 text-[10px] font-medium px-2 py-0.5 rounded-full` (inline style: color, bg, border)

## Dropdown Containers (Tag/Label)
- [ ] **Dropdown Wrapper** — `app/views/email_messages/_tags.html.erb:28` — `hidden absolute top-full left-0 z-50 mt-1 bg-white border border-gray-200 rounded-lg shadow-lg` with inline `min-width:180px;max-height:200px;overflow-y:auto`
  also at: `app/views/email_messages/_zoho_labels.html.erb:28`
- [ ] **Dropdown Results** — `app/views/email_messages/_tags.html.erb:35` — `py-1`

## Selection / Checkbox Areas
- [ ] **Checkbox Slide-in (hover reveal)** — `app/views/email_messages/_thread_list.html.erb:20` — `flex-shrink-0 w-0 group-hover:w-6 overflow-hidden transition-all duration-150 flex items-center justify-center relative z-10 self-stretch pl-2`
- [ ] **Select-all Header Bar** — `app/views/email_messages/index.html.erb:59` — `px-3 py-1.5 border-b border-gray-100 flex items-center gap-2 opacity-0 hover:opacity-100 transition-opacity`
- [ ] **Selection Toolbar (floating)** — `app/views/email_messages/index.html.erb:83` — `hidden absolute top-12 left-1/2 -translate-x-1/2 z-20 bg-white shadow-lg border border-gray-200 rounded-lg px-4 py-2 flex items-center gap-3`
- [ ] **Selection Count** — `app/views/email_messages/index.html.erb:84` — `text-xs font-medium text-gray-700`

## Modal (Inbox Settings)
- [x] **Modal Done Button** — `app/views/email_messages/show.html.erb:308` — used `Campbooks::Button` with variant: :primary

## Empty States
- [x] **Empty State (centered, full page)** — `app/views/email_messages/empty.html.erb:1` — `flex items-center justify-center h-[calc(100dvh-4rem)] bg-gray-50/30`
- [x] **Empty State (no email selected)** — `app/views/email_messages/index.html.erb:92` — centered with small icon + `text-[11px] font-medium text-gray-500`
- [x] **Empty State (no emails in folder)** — `app/views/email_messages/index.html.erb:77` — `p-6 text-center text-[11px] text-gray-400`
- [x] **Empty State (no chat messages)** — `app/views/email_messages/show.html.erb:251` — `px-4 py-8 text-center text-[11px] text-gray-400`
- [x] **Empty State (no scans, table row)** — `app/views/email_scans/index.html.erb:43` — `px-6 py-8 text-center text-sm text-gray-500`
  also at: `app/views/email_sync/show.html.erb:77,140`

## Loading/Spinner
- [x] **Inline Spinner** — `app/views/email_messages/index.html.erb:72` — `animate-spin w-4 h-4 border-2 border-gray-300 border-t-accent-500 rounded-full`
  also at: `app/views/email_messages/index.turbo_stream.erb:9`

## Chat Panel
- [x] **Chat Panel Header Button** — `app/views/email_messages/show.html.erb:217` — IconButton with toggle

## Typing Indicator
- [x] **Typing Container** — `app/views/email_comments/_typing.html.erb:1` — `flex items-start gap-3 px-4 py-3`
- [x] **Typing Dots** — `app/views/email_comments/_typing.html.erb:13-15` — `w-1.5 h-1.5 rounded-full bg-blue-400` with staggered animation
- [x] **Typing Animation** — `app/views/email_comments/_typing.html.erb:21-24` — `@keyframes typingBounce`

## Page Headers
- [x] **Page Header** — `app/views/email_accounts/index.html.erb` — Using `Campbooks::PageHeader`
- [x] **Page Header** — `app/views/email_accounts/new.html.erb` — Using `Campbooks::PageHeader`
- [x] **Page Header** — `app/views/email_scans/index.html.erb` — Using `Campbooks::PageHeader`
- [x] **Page Header** — `app/views/email_sync/show.html.erb` — Using `Campbooks::PageHeader`

## Quick Links
- [x] **Quick Links Grid** — `app/views/email_sync/show.html.erb:152` — Using `Campbooks::Card` with hover

## Back Links
- [x] **Back Link** — `app/views/email_accounts/show.html.erb:4` — Using `Campbooks::Button` with variant: :ghost

## Tables (kept as raw HTML - Table component uses instance_exec which breaks view helper access)
- [ ] **Admin Table** — `app/views/email_scans/index.html.erb:7` — `min-w-full divide-y divide-gray-200`
  also at: `app/views/email_scans/show.html.erb:57`, `app/views/email_sync/show.html.erb:41,98`
