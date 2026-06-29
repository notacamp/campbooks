# frozen_string_literal: true

module Api
  # App-facing catalog of public-API OAuth scopes: names + human descriptions,
  # used by the Settings → API access scope picker and the API docs. The
  # canonical *list* of enabled scopes lives in config/initializers/doorkeeper.rb
  # (Doorkeeper `optional_scopes`); the spec spec/models/api/scopes_spec.rb
  # asserts this catalog matches it, so the two never drift.
  module Scopes
    # scope name (String) => short, human-readable description
    CATALOG = {
      "emails:read"     => "Read email messages, threads, and folders",
      "emails:write"    => "Mark emails read/unread",
      "emails:send"     => "Compose, send, and reply to email",
      "documents:read"  => "List and download documents",
      "documents:write" => "Upload, update, approve, reject, and reclassify documents",
      "contacts:read"   => "Read contacts",
      "contacts:write"  => "Update contacts and change their state (star, block, allow)",
      "tags:read"       => "List tags",
      "tags:write"      => "Add and remove tags on emails",
      "document_types:read" => "List document types",
      "workflows:read"     => "List workflows and their run history",
      "workflows:trigger"  => "Trigger a workflow",
      "scout:read"         => "Read Scout chat threads and messages",
      "scout:write"        => "Create Scout threads and send messages",
      "scheduled_emails:read"  => "List and read scheduled emails",
      "scheduled_emails:write" => "Schedule, update, and cancel emails",
      "calendar:read"          => "Read calendar events",
      "calendar:write"         => "Create, update, RSVP, and delete calendar events",
      "reminders:read"         => "Read AI reminders",
      "reminders:write"        => "Confirm, dismiss, and snooze reminders",
      "tasks:read"             => "List and read tasks",
      "tasks:write"            => "Create, update, and complete tasks",
      "folders:read"           => "List folders and their contents",
      "folders:write"          => "File and unfile documents in folders",
      "templates:read"         => "List email templates",
      "templates:write"        => "Create, update, and delete email templates"
    }.freeze

    module_function

    # All enabled scope names, as strings.
    def all
      CATALOG.keys
    end

    def description(scope)
      CATALOG[scope.to_s]
    end

    # Reduce a user-supplied scope selection (an array of names, and/or a
    # space-delimited string) to the recognized scopes only. Returns an array.
    def sanitize(requested)
      Array(requested).flat_map { |value| value.to_s.split }.uniq & all
    end
  end
end
