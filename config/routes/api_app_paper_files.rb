# frozen_string_literal: true

# Paper (document buckets), Files (manager/uploads/shares/public links), document+email Skim, Search, Organizations. See api-migration/paper-files.md.
# Owned by the api_app_paper_files build agent — add routes only under the namespaces below;
# do NOT edit config/routes.rb or other api_app_*.rb files. Session-bearer auth
# via Api::App::BaseController. Controllers live in app/controllers/api/app/.
namespace :api do
  namespace :app do
    # Paper
    get "paper", to: "paper/paper#index"

    # Documents — collection actions first (before :id member routes to avoid ambiguity)
    post   "documents/reprocess_all", to: "documents/documents#reprocess_all"
    post   "documents/export",        to: "documents/documents#export"
    get    "documents/merge",         to: "documents/documents#merge"
    post   "documents/perform_merge", to: "documents/documents#perform_merge"
    resources :documents, only: %i[show create update], module: "documents" do
      member do
        get    "file"
        patch  "rename"
        post   "approve"
        post   "reject"
        patch  "toggle_star"
        post   "reprocess"
        post   "settle"
        delete "settle", action: :unsettle
        post   "push_to_drive"
        post   "push_to_notion"
        post   "push_to_zoho_drive"
      end
    end

    # Document Skim
    scope "document_skim", module: "document_skim" do
      get  "/",                to: "skim#show",         as: :document_skim
      get  "tray",             to: "skim#tray",         as: :document_skim_tray
      post ":id/approve",      to: "skim#approve",      as: :document_skim_approve
      patch ":id/reclassify",  to: "skim#reclassify",   as: :document_skim_reclassify
      patch ":id/update_fields", to: "skim#update_fields", as: :document_skim_update_fields
      post ":id/reprocess",    to: "skim#reprocess",    as: :document_skim_reprocess
      post ":id/dismiss",      to: "skim#dismiss",      as: :document_skim_dismiss
      post ":id/restore",      to: "skim#restore",      as: :document_skim_restore
    end

    # Email Skim
    scope "email_skim", module: "email_skim" do
      get  "/",                      to: "skim#show",              as: :email_skim
      get  "tray",                   to: "skim#tray",              as: :email_skim_tray
      post "decide",                 to: "skim#decide",            as: :email_skim_decide
      post "undo",                   to: "skim#undo",              as: :email_skim_undo
      post "keep",                   to: "skim#keep",              as: :email_skim_keep
      post "promote",                to: "skim#promote",           as: :email_skim_promote
      post "unpromote",              to: "skim#unpromote",         as: :email_skim_unpromote
      post "dismiss_follow_up",      to: "skim#dismiss_follow_up", as: :email_skim_dismiss_follow_up
      post "sender_action",          to: "skim#sender_action",     as: :email_skim_sender_action
      get  "emails/:id",             to: "skim#email",             as: :email_skim_email
      post "emails/:id/reply",       to: "skim#reply",             as: :email_skim_reply
    end

    # Files
    get    "files",                        to: "files/files#index",   as: :api_app_files
    get    "files/folders/:id",            to: "files/files#show",    as: :api_app_files_folder
    post   "files/uploads",                to: "files/uploads#create", as: :api_app_files_uploads
    delete "files/uploads/:id",            to: "files/uploads#destroy", as: :api_app_files_upload
    post   "files/uploads/:id/analyze",    to: "files/uploads#analyze", as: :api_app_files_upload_analyze
    post   "files/public_links",           to: "files/public_links#create", as: :api_app_files_public_links
    delete "files/public_links/:id",       to: "files/public_links#destroy", as: :api_app_files_public_link
    get    "files/public_links/picker",    to: "files/public_links#picker", as: :api_app_files_public_links_picker

    # Mail Folders
    resources :mail_folders, only: %i[show create update destroy], module: "files"

    # Folder Memberships
    resources :folder_memberships, only: %i[create destroy], module: "files"

    # Search
    get "search", to: "search/search#index", as: :api_app_search

    # Organizations
    resources :organizations, only: %i[index show update], module: "organizations" do
      collection do
        post "backfill"
      end
      member do
        get "emails"
        get "documents"
      end
    end
  end
end
