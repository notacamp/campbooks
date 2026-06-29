Rails.application.routes.draw do
  # Public REST API OAuth endpoints. Mounted under /api/oauth (NOT the default
  # /oauth) so they never collide with the inbound provider callbacks in
  # `namespace :oauth` below.
  #   POST    /api/oauth/token      — exchange client_id/secret (or an auth code) for a token
  #   POST    /api/oauth/revoke     — revoke a token
  #   GET/POST/DELETE /api/oauth/authorize — browser SSO consent (authorization_code + PKCE)
  # The authorize endpoint uses a styled custom view + the app's cookie session
  # (Oauth::AuthorizationsController). The applications-admin, authorized-apps, and
  # token-introspection controllers stay disabled.
  scope "api" do
    use_doorkeeper scope: "oauth" do
      controllers authorizations: "oauth/authorizations"
      skip_controllers :applications, :authorized_applications, :token_info
    end
  end

  resource :session do
    get :zoho
    get :google
    get :microsoft
    get :native
    # Second-factor (2FA) step after a correct password; OAuth/native bypass it.
    resource :challenge, only: [ :show, :create ], controller: "session_challenges" do
      get :passkey_options    # WebAuthn assertion options (JSON)
      post :send_email_code   # dispatch the email one-time code
    end
  end
  resources :passwords, param: :token

  resource :registration, only: [ :new, :create ] do
    member do
      get :verify
      post :check_code
      post :resend_code
      get :password
      patch :complete
      get :pending_approval
    end
  end

  get "registration/approved", to: "registrations#approved", as: :registration_approved

  # Public invitation acceptance
  get "invitations/:token", to: "invitations#show", as: :invitation
  post "invitations/:token/accept", to: "invitations#accept", as: :accept_invitation

  resource :onboarding, only: [ :show, :update ], controller: "onboarding" do
    member do
      post :snooze
      post :suggest_document_types
      post :suggest_tags
    end
  end

  get "setup/:id", to: "setup#show", as: :setup
  patch "setup/:id", to: "setup#update"
  post "setup/dismiss", to: "setup#dismiss", as: :dismiss_setup

  # One-time guided overlays ("tours", e.g. the skim intro). The client POSTs the
  # tour key here once the user has seen it so it won't reappear.
  post "tours/:key/dismiss", to: "tours#dismiss", as: :dismiss_tour

  # Conversational AI setup — Scout asks a few questions, then proposes Document
  # Types / Tags the user accepts. Keyed by :kind (document_types|tags); the
  # thread is the one setup_chat AgentThread per (workspace, user, kind).
  get  "ai_setup/:kind",         to: "ai_setup_chats#show",    as: :ai_setup_chat
  post "ai_setup/:kind",         to: "ai_setup_chats#create",  as: :start_ai_setup_chat
  post "ai_setup/:kind/message", to: "ai_setup_chats#message", as: :ai_setup_chat_message
  post "ai_setup/:kind/apply",   to: "ai_setup_chats#apply",   as: :ai_setup_chat_apply

  root "home#index"
  get "home", to: "home#index", as: :home

  # Workspace activity feed — a retrospective timeline of domain Events (distinct
  # from the prospective home feed). Read-only; turbo_stream serves pagination.
  get "activity", to: "activity#index", as: :activity

  # Home-feed item interactions. The feed itself is rendered by home#index; these
  # act on a single card: run its suggested action, dismiss it, or mark it seen.
  namespace :feed do
    resources :items, only: [] do
      member do
        post :act
        post :dismiss
        post :seen
        post :undo
      end
    end
  end

  # AI-extracted reminders awaiting confirmation into calendar events.
  resources :reminders, only: [ :index ] do
    member do
      post :confirm
      post :dismiss
      post :snooze
    end
  end

  # Tasks — actionable items (manual or AI-extracted) that move through a status
  # board, carry assignees + labels, and link to emails. Gated by Features.tasks?
  # and the :tasks entitlement (TasksController). Board + skim + email-linking
  # routes are added in their respective phases.
  resources :tasks do
    member do
      patch  :complete        # mark done (quick action)
      patch  :move            # change status (status control + the board drag)
      post   :assign          # update assignees
      get    :email_picker    # search emails to link (turbo-frame)
      post   :link_email      # link an existing email to this task
      delete :unlink_email    # remove a task↔email link
      get    :document_picker # search documents to attach (turbo-frame)
      post   :attach_document # attach a workspace document
      delete :detach_document # remove an attached document
      post   :remind          # create a linked deadline reminder
      patch  :accept          # suggested → todo (Skim triage)
      patch  :dismiss         # suggested → cancelled (Skim triage)
      patch  :archive         # soft-archive (hide without deleting)
      patch  :unarchive       # restore an archived task
    end
    collection do
      get :skim               # triage AI-suggested tasks
      get :board              # status kanban
    end

    # Discussion thread (teammates + Scout on @scout).
    resources :comments, only: [ :create ], controller: "tasks/comments" do
      collection { get :poll }
    end
  end

  # Global search (Cmd+K command palette)
  get "search", to: "search#index"

  # Skim Mode — inbox grouped into cluster cards, reviewed one stack at a time
  get "skim", to: "skim#show"
  get "skim/tray", to: "skim#tray", as: :skim_tray
  post "skim/decide", to: "skim#decide"
  post "skim/undo", to: "skim#undo"
  post "skim/keep", to: "skim#keep"
  post "skim/promote", to: "skim#promote"
  post "skim/unpromote", to: "skim#unpromote"
  post "skim/dismiss_follow_up", to: "skim#dismiss_follow_up"
  post "skim/sender_action", to: "skim#sender_action"
  get "skim/email/:id", to: "skim#email", as: :skim_email
  get "skim/email/:id/content", to: "skim#email_content", as: :skim_email_content
  post "skim/email/:id/reply", to: "skim#reply", as: :skim_email_reply

  # Skim for Documents — the review queue grouped into category rings, reviewed one
  # document at a time (mirrors email Skim). Declared BEFORE `resources :documents`
  # so "/documents/skim" isn't captured by documents#show (:id = "skim").
  get  "documents/skim",      to: "documents/skim#show",  as: :document_skim
  get  "documents/skim/tray", to: "documents/skim#tray",  as: :document_skim_tray
  post  "documents/skim/:id/approve",       to: "documents/skim#approve",       as: :approve_document_skim
  patch "documents/skim/:id/reclassify",    to: "documents/skim#reclassify",    as: :reclassify_document_skim
  patch "documents/skim/:id/update_fields", to: "documents/skim#update_fields", as: :update_fields_document_skim
  post  "documents/skim/:id/reprocess",     to: "documents/skim#reprocess",     as: :reprocess_document_skim
  post  "documents/skim/:id/dismiss",       to: "documents/skim#dismiss",       as: :dismiss_document_skim
  post  "documents/skim/:id/restore",       to: "documents/skim#restore",       as: :restore_document_skim

  # Document writing tool — author formatted documents from scratch.
  # Declared BEFORE `resources :documents` to avoid :id = "write" capture.
  get   "documents/write",      to: "documents/written#index", as: :written_documents
  get   "documents/write/new",  to: "documents/written#new",  as: :new_written_document
  post  "documents/write",      to: "documents/written#create"
  get   "documents/write/:id",  to: "documents/written#show", as: :written_document
  get   "documents/write/:id/edit", to: "documents/written#edit", as: :edit_written_document
  patch "documents/write/:id",  to: "documents/written#update"

  resources :documents, only: [ :index, :show, :update, :create ] do
    member do
      get :file
      patch :rename
      post :approve
      post :reject
      patch :toggle_star
      post :reprocess
      post :push_to_notion
      post :push_to_drive
      post :push_to_zoho_drive
    end
    # Interactive, on-demand exports (full-page wizards with a Turbo-Frame browser).
    resource :drive_export, only: [ :new, :create ], controller: "documents/drive_exports" do
      post :create_folder
    end
    resource :notion_export, only: [ :new, :create ], controller: "documents/notion_exports" do
      get :databases
      get :database_form
      get :pages
    end
    collection do
      post :reprocess_all
      post :export
      get :merge
      post :perform_merge
    end
  end

  # User-submitted bug reports (in-app "Report a bug" widget).
  resources :bug_reports, only: [ :create ]

  # Browser CSP violation reports (the policy's report-uri target).
  post "csp-reports" => "security/csp_reports#create"

  # Global AI Agent Chat
  resource :scout, only: [ :show, :create ], controller: "agent_chat" do
    resources :threads, only: [ :show, :create, :update, :destroy ], controller: "agent_threads" do
      resources :messages, only: [ :create ], controller: "agent_messages"
    end
  end
  post "scout/tool", to: "agent_tools#create", as: :scout_tool

  resources :email_accounts, only: [ :create, :update, :destroy ] do
    member do
      get :sharing
      get :popover
    end
    resources :email_folders, only: [] do
      collection do
        patch :reorder
      end
    end
  end
  resources :calendar_accounts, only: [ :update, :destroy ] do
    member do
      get :sharing
    end
    # Per-calendar sync toggle / color override within a connected account.
    resources :calendars, only: [ :update ]
  end

  resources :email_scans, only: [ :show ]

  resources :zoho_drive_accounts, only: [ :new, :create, :destroy ]

  resources :scheduled_emails

  resources :workflows do
    member do
      post :toggle
      post :add_step
      post :regenerate_webhook
      delete "steps/:step_id", to: "workflows#remove_step", as: :step
    end
    collection do
      # Renders the per-field Notion config form for a chosen database (Turbo Frame).
      get :notion_fields
    end
    resources :executions, only: [ :index, :show ], controller: "workflow_executions"
  end

  # Public inbound webhook — external services POST here to trigger a workflow.
  # The :token path segment is the shared secret; no session auth.
  match "webhooks/:token", to: "webhooks#receive", as: :webhook, via: [ :post, :put ]

  # Public push receiver for Google Calendar watch channels (no session auth).
  post "calendar_webhooks/google", to: "calendar_webhooks#google_receive", as: :calendar_webhooks_google

  namespace :settings do
    root to: "general#show"
    resource :general, only: [ :show, :update ], controller: "general"
    resource :plan, only: [ :show ], controller: "plan"
    resource :ai, only: [ :show ], controller: "ai" do
      post :switch_mode
    end
    resource :data_privacy, only: [ :show, :update ], controller: "data_privacy"
    resource :account, only: [ :show, :update, :destroy ], controller: "account" do
      patch :language
      patch :writing_style
      post :analyze_writing_style
      get :delete
      post :export
      get :download_export
    end
    # Two-factor authentication management (opt-in second factors).
    resource :security, only: [ :show ], controller: "security" do
      delete :disable # turn off all 2FA (requires password re-auth)
      resource :totp, only: [ :new, :create, :destroy ], controller: "security/totp"
      resources :passkeys, only: [ :new, :create, :destroy ], controller: "security/passkeys" do
        get :options, on: :collection # WebAuthn creation options (JSON)
      end
      resource :recovery_codes, only: [ :show, :create ], controller: "security/recovery_codes"
      resource :email_otp, only: [ :create, :destroy ], controller: "security/email_otp"
      # OAuth sign-in methods (create → provider consent for flow=add_sign_in;
      # destroy → unlink an Identity). The provider callbacks land back in Oauth::*.
      resources :sign_in_methods, only: [ :create, :destroy ], controller: "security/sign_in_methods"
      # Read-only personal security/audit history (sign-ins, MFA changes, exports…).
      get "audit_log", to: "security/audit_log#index"
    end
    resources :members, only: [ :index ]
    resources :invitations, only: [ :create, :destroy ] do
      member do
        post :resend
      end
    end
    resources :notifications, only: [ :index ]

    # Developer API access — manage OAuth clients (Doorkeeper applications) that
    # call the public REST API. The client secret is shown once at create/rotate.
    resources :api_clients, only: [ :index, :new, :create, :destroy ] do
      member do
        post :regenerate_secret
        post :revoke
      end
    end

    resources :document_templates, except: :show do
      member { post :regenerate }
    end

    namespace :integrations do
      root to: "index#show"
      resource :notion, only: [ :show, :update ], controller: "notion"
      # Disconnect a specific connected Notion workspace (multi-workspace via OAuth).
      delete "notion/integrations/:id", to: "notion#destroy", as: :notion_integration
      resource :google_drive, only: [ :show, :destroy ], controller: "google_drive" do
        post :retry_failed
        get "configs/:document_type_id/edit", to: "google_drive_configs#edit", as: :edit_config
        get "configs/:document_type_id/folders", to: "google_drive_configs#browse_folders", as: :browse_folders_config
        patch "configs/:document_type_id", to: "google_drive_configs#update", as: :config
      end
      resource :zoho_drive, only: [ :show, :update, :destroy ], controller: "zoho_drive"
      resource :calendars, only: [ :show ], controller: "calendars"
      resources :connections, only: [ :index, :new, :create, :edit, :update, :destroy ]
    end

    resources :pipelines, except: [ :show ]
  end

  # Pipelines kanban board (outside Settings).
  resources :pipelines, only: [] do
    member do
      get :board, to: "pipeline_board#index"
      post :move, to: "pipeline_board#move"
    end
    # Add/remove documents & emails to the board (the item picker).
    resources :memberships, only: [ :new, :create, :destroy ], controller: "pipeline_memberships"
  end

  resources :document_templates, only: [] do
    member { get :fill; post :preview; post :send_email }
  end

  # Inbox settings — the gear-icon management modal on the email page. Each
  # action renders a panel into the modal's Turbo Frame (inbox_settings_panel).
  namespace :inbox_settings do
    get "display", to: "display#show",   as: :display
    patch "display", to: "display#update", as: :display_update

    # Inbox filtering strategy (whitelist/blacklist) + blocked/starred/allowed
    # sender management.
    get   "filtering",        to: "filtering#show",       as: :filtering
    patch "filtering",        to: "filtering#update"
    post  "filtering/sender", to: "filtering#set_sender", as: :filtering_sender

    resources :tags, except: [ :show ]
    resources :document_types, except: [ :show ]
    resources :signatures, except: [ :show ] do
      member { post :set_default }
    end

    resources :external_labels, only: [ :index, :create, :update, :destroy ] do
      collection { post :sync }
    end

    get  "accounts", to: "accounts#show", as: :accounts
    post "accounts/scan", to: "accounts#scan_now", as: :accounts_scan
  end

  # Contacts — a first-class people directory + profile pages, plus the app-wide
  # hover card and the compose autocomplete/lookup endpoints. Promoted out of the
  # inbox-settings modal into the primary nav. `resources` emits the collection
  # routes (popover/lookup/search/…) before /contacts/:id, so the static paths
  # resolve ahead of the :id catch-all. The popover Stimulus controller fetches
  # /contacts/:id/popover and /contacts/popover?email=… ; ContactPillInput's
  # compose autocomplete hits /contacts/search.
  resources :contacts, only: [ :index, :show, :update ] do
    member do
      get  :popover
      post :analyze
      post :resolve_duplicate
      post :set_state          # star / unstar / block / unblock / allow
    end
    collection do
      get  :popover
      get  :lookup
      get  :search
      get  :skim               # Tinder-style triage of new/undecided senders
      post :scan_dedup
      post :consolidate_all
    end
  end


    resources :organizations, only: [ :index, :show, :update ] do
    member do
      get :emails
      get :documents
    end
    collection do
      post :backfill
    end
  end

  resources :email_threads, only: [ :index, :show ]

  # User-defined inbox folders (chips on top of the inbox). Creating one
  # provisions a real folder/label on every connected account.
  resources :mail_folders, only: [ :show, :create, :update, :destroy ]

  # Filing content (documents now) into custom folders — the Stage 3 "filesystem" layer.
  resources :folder_memberships, only: [ :create, :destroy ]

  # Files — the native file area: a unified file manager over uploaded files and
  # (later) internal documents + emails filed into custom folders (MailFolder).
  # Folder CRUD + filing reuse mail_folders / folder_memberships above; only the
  # light-path upload (store without the AI pipeline) is Files-specific.
  get "files", to: "files#index", as: :files
  get "files/folders/:id", to: "files#show", as: :files_folder
  namespace :files do
    resources :uploads, only: [ :create, :destroy ] do
      member { post :analyze }
    end
    # Per-folder sharing (Phase 3): restrict a folder + manage members.
    resources :folders, only: [] do
      resource :share, only: [ :show, :update ], controller: "folder_shares"
    end
  end

  get "email_messages/new", to: "email_messages#new", as: :new_email_message
  post "email_messages/compose_chat", to: "email_compose_chat#create"

  # Inline image upload for the compose / signature rich-text editor. Stores the
  # image and returns a stable, app-served (proxy) URL recipients can load.
  post "compose_images", to: "compose_images#create", as: :compose_images
  # File-attachment upload for the composer. Stores the file and returns a signed
  # blob id the compose form carries; resolved + attached to the mail at send.
  post "compose_attachments", to: "compose_attachments#create", as: :compose_attachments

  resources :email_messages, only: [ :index, :show ] do
    collection do
      get :search
      post :send_new, to: "email_compose#send_message"
      get :board, to: "email_messages/board#index"
      post :board_move, to: "email_messages/board#move"
    end
    collection do
      post :bulk, to: "email_messages/bulk#create"
    end
    member do
      get :drawer_content
      get :folders, to: "email_messages/folders#index"
      post :tool, to: "email_tools#create"
      patch :dismiss_todo
      post   "follow", to: "thread_follows#create", as: :follow
      delete "follow", to: "thread_follows#destroy"
      post :compose, to: "email_compose#create"
      post :send_message, to: "email_compose#send_message"
      post :discard_compose, to: "email_compose#discard"
    end
    # Interactive "Save email to Notion" (attachments + subject/body as the source).
    resource :notion_export, only: [ :new, :create ], controller: "email_messages/notion_exports" do
      get :databases
      get :database_form
      get :pages
    end
    resources :comments, only: [ :create ], controller: "email_comments" do
      collection do
        get :poll
      end
    end
    resources :tags, only: [ :create, :destroy ], controller: "email_message_tags"
    resources :labels, only: [ :create, :destroy ], controller: "email_message_zoho_labels"
    resources :zoho_labels, only: [ :create, :destroy ], controller: "email_message_zoho_labels"
  end

  # Calendar — agenda + month/week views over synced events, with two-way event
  # CRUD/RSVP. The page reads ?view=agenda|week|month and ?date=YYYY-MM-DD.
  get "calendar", to: "calendar#index", as: :calendar
  resources :calendar_events, only: [ :show, :new, :create, :edit, :update, :destroy ] do
    member do
      post :rsvp
      patch :reschedule # drag-to-reschedule from the day/week time grids
    end
  end

  # Calendar-only "tags": name + color + AI prompt used to auto-classify and color
  # events. Managed from the calendar toolbar (turbo-frame inline form CRUD).
  resources :event_types, except: [ :show ] do
    collection { post :starters } # one-click starter set from the empty state
  end

  # ── Public REST API (v1) ──────────────────────────────────────────────────
  # Authenticated with OAuth bearer tokens minted at /api/oauth/token. Every
  # controller inherits Api::V1::BaseController (token auth + workspace/acting-user
  # bridge + JSON envelope). Resource paths use customer-friendly names (/emails)
  # even where the underlying model differs (EmailMessage).
  namespace :api do
    namespace :v1 do
      # GET /api/v1/me — the identity behind the token (acting user + workspace +
      # granted scopes). Needs only a valid token, so the CLI can confirm login.
      get "me", to: "me#show"

      resources :emails, only: [ :index, :show, :create ], controller: "email_messages" do
        member do
          post :mark_read
          post :mark_unread
          post :reply
        end
        # POST /api/v1/emails/:email_id/tags  ·  DELETE /api/v1/emails/:email_id/tags/:id
        resources :tags, only: [ :create, :destroy ], controller: "email_tags"
      end

      resources :documents, only: [ :index, :show, :create, :update ] do
        member do
          get :file
          post :approve
          post :reject
          post :reclassify
        end
      end

      resources :contacts, only: [ :index, :show, :update ] do
        member do
          post :state # star / unstar / allow / block / unblock
        end
      end

      resources :tags, only: [ :index ]
      resources :document_types, only: [ :index ]

      resources :workflows, only: [ :index ] do
        member { post :trigger }
        resources :executions, only: [ :index ], controller: "workflow_executions"
      end

      # Scout AI chat. Async: post a message, then poll the messages endpoint for
      # the AI reply (?after_message_id=N).
      scope "scout", as: "scout" do
        resources :threads, only: [ :index, :create ], controller: "scout_threads" do
          resources :messages, only: [ :index, :create ], controller: "scout_messages"
        end
      end

      # Scheduled (and recurring) email sends. destroy = cancel (status only).
      resources :scheduled_emails, only: [ :index, :show, :create, :update, :destroy ]

      # Calendar events. Writes ride the same provider write-through job as the web.
      resources :calendar_events, only: [ :index, :show, :create, :update, :destroy ] do
        member { post :rsvp }
      end

      # AI-extracted reminders. Read + state transitions only (no manual create).
      resources :reminders, only: [ :index, :show ] do
        member do
          post :confirm
          post :dismiss
          post :snooze
        end
      end

      # Tasks: list/read, create, update, and complete (status transitions publish
      # the same domain events as the web UI via Task#move_to_status!).
      resources :tasks, only: [ :index, :show, :create, :update ] do
        member { patch :complete }
      end

      # Custom folders (MailFolder) + filing documents into them. No provider-side
      # folder create/rename over the API (per-account side effects).
      resources :folders, only: [ :index, :show ], controller: "folders"
      resources :folder_memberships, only: [ :create, :destroy ]
    end

    # MCP (Model Context Protocol) JSON-RPC endpoint. Protocol-versioned via the
    # initialize handshake, not URL-versioned, so it sits beside v1 (and the
    # /api/oauth token endpoint), not under it. Same bearer-token auth.
    post "mcp", to: "mcp#create"
  end

  namespace :oauth do
    get "zoho/callback", to: "zoho#callback"
    get "google/connect", to: "google#connect"
    get "google/callback", to: "google#callback"
    get "gmail/callback", to: "google_mail#callback"
    get "microsoft/callback", to: "microsoft#callback"
    get "notion/connect", to: "notion#connect"
    get "notion/callback", to: "notion#callback"
  end

  resources :notifications, only: [ :index, :show, :destroy ] do
    member do
      post :mark_read
      post :archive
      post :unarchive
    end
    collection do
      post :mark_all_read
      post :archive_all
    end
  end

  # Native push-token registration: the iOS/Android shell POSTs its device token
  # here (create) and removes it on sign-out/permission-revoke (destroy by token).
  resource :device, only: [ :create, :destroy ]

  patch "notification_preferences/toggle", to: "notifications#toggle_preference", as: :toggle_notification_preferences
  patch "notification_preferences/bulk_toggle", to: "notifications#bulk_toggle", as: :bulk_toggle_notification_preferences

  resources :ai_configurations, only: [ :update ]

  namespace :settings do
    resources :ai_adapters, only: [ :create, :update, :destroy ]
  end

  namespace :admin do
    root to: "dashboard#show"
    resources :signup_requests, only: [ :index ] do
      member do
        post :approve
        post :reject
      end
    end
    resources :invitations, only: [ :index ] do
      member do
        post :approve
        post :reject
      end
    end
    resources :users, only: [ :index, :update ]
    resources :beta_codes, only: [ :index, :create, :destroy ]
  end

  mount MissionControl::Jobs::Engine, at: "/jobs"

  get "email_images/:email_account_id/(*path)", to: "email_images#show", as: :email_image

  mount Lookbook::Engine, at: "/lookbook" if Rails.env.development?

  # Hotwire Native path configuration — the navigation ruleset the native iOS/
  # Android shells fetch at launch. Public + versioned per platform (ios_v1,
  # android_v1, …) so installed apps can roll forward independently.
  get "configurations/:platform", to: "configurations#show", as: :path_configuration

  # Liveness probe. Custom controller (mirrors rails/health#show) that also
  # reports Campbooks::VERSION so operators can see what's deployed. Keeps the
  # path and route name so the production SSL exclusion and any path helpers
  # continue to work.
  get "up" => "health#show", as: :rails_health_check
end
