# frozen_string_literal: true

require "rails_helper"

# Engine-level integration tests for rule-based group membership in
# Emails::TagGroups. Covers all four rule types, multi-group additive
# membership across both tag- and rule-based groups, and verifies that
# the human-care guards still apply to rule-matched threads.
RSpec.describe Emails::TagGroups, "rule-based membership" do
  before do
    @workspace = Workspace.create!(name: "Rules Svc WS #{SecureRandom.hex(4)}")
    @account   = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    @user = @workspace.users.create!(
      name: "Ana", email_address: "ana-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
    @account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
    Tags::DefaultGroups.provision!(@workspace)
    @promo_tag = Tags::DefaultGroups.bucket_tag_for(@workspace, "promotions")
  end

  # Helper: service configured for the test account.
  def service
    described_class.new(@workspace, [ @account.id ])
  end

  # Helper: create a thread with one message. Uses a unique from address per call
  # to avoid contact email uniqueness collisions.
  def create_thread(subject: "T", from: nil, contact: nil,
                    tags: [], category: nil, doc_type: nil)
    from ||= "sender-#{SecureRandom.hex(4)}@bulk.test"
    thread  = @account.email_threads.create!(subject: subject)
    contact ||= @workspace.contacts.find_or_create_by!(email: from)
    msg     = @account.email_messages.create!(
      email_thread:        thread,
      provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id:  "INBOX",
      from_address:        from,
      to_address:          @account.email_address,
      subject:             subject,
      received_at:         1.hour.ago,
      read:                false,
      has_attachment:      false,
      category:            category,
      contact:             contact
    )
    Array(tags).each { |t| msg.email_message_tags.create!(tag: t) }

    if doc_type
      # Bypass the original_file presence validation (test-only: we care only
      # about document_type_id for the rule engine, not the file attachment).
      doc_id = SecureRandom.uuid
      now    = Time.current.utc.iso8601(6)
      ApplicationRecord.connection.execute(<<~SQL)
        INSERT INTO documents (
          id, workspace_id, document_type_id, email_account_id, canonical_filename,
          document_type, ai_status, review_status, source, google_drive_push_status,
          created_at, updated_at
        ) VALUES (
          '#{doc_id}', '#{@workspace.id}', '#{doc_type.id}', '#{@account.id}',
          'doc-#{SecureRandom.hex(4)}.pdf', 0, 0, 0, 0, 0, '#{now}', '#{now}'
        )
      SQL
      DocumentEmailMessage.create!(
        document_id: doc_id, email_message_id: msg.id
      )
    end

    thread
  end

  # Helper: create a rule for the given group.
  def add_rule(group_name, rule_type, value)
    @workspace.inbox_group_rules.create!(
      group_name: group_name,
      rule_type:  rule_type,
      value:      value
    )
  end

  # ---- sender rule (exact email) -------------------------------------------

  describe "sender rule (exact email address)" do
    it "includes threads from a matching sender" do
      addr = "billing-#{SecureRandom.hex(4)}@vendor.example"
      add_rule("Vendors", "sender", addr)
      thread = create_thread(from: addr)
      expect(service.group_scope("Vendors").where(id: thread.id).exists?).to be(true)
    end

    it "does not include threads from a different sender" do
      add_rule("Vendors", "sender", "billing-#{SecureRandom.hex(4)}@vendor.example")
      thread = create_thread  # default random from address
      expect(service.group_scope("Vendors").where(id: thread.id).exists?).to be(false)
    end

    it "matches case-insensitively" do
      addr = "billing-#{SecureRandom.hex(4)}@vendor.example"
      add_rule("Vendors", "sender", addr.upcase)
      thread = create_thread(from: addr.downcase)
      expect(service.group_scope("Vendors").where(id: thread.id).exists?).to be(true)
    end
  end

  # ---- sender rule (@domain) ------------------------------------------------

  describe "sender rule (@domain)" do
    it "includes all threads whose sender is in the domain" do
      add_rule("Acme", "sender", "@acme.example")
      t1 = create_thread(from: "alice-#{SecureRandom.hex(4)}@acme.example")
      t2 = create_thread(from: "bob-#{SecureRandom.hex(4)}@acme.example")
      t3 = create_thread(from: "carol-#{SecureRandom.hex(4)}@other.example")

      scope = service.group_scope("Acme")
      expect(scope.where(id: t1.id).exists?).to be(true)
      expect(scope.where(id: t2.id).exists?).to be(true)
      expect(scope.where(id: t3.id).exists?).to be(false)
    end

    it "does not match the domain as a substring of a longer domain" do
      add_rule("Acme", "sender", "@acme.example")
      # "notacme.example" should not match "@acme.example"
      t = create_thread(from: "alice-#{SecureRandom.hex(4)}@notacme.example")
      expect(service.group_scope("Acme").where(id: t.id).exists?).to be(false)
    end
  end

  # ---- organization rule ---------------------------------------------------

  describe "organization rule" do
    it "includes threads from a contact belonging to the organization" do
      person = @workspace.people.create!(name: "Alice")
      org    = @workspace.organizations.create!(name: "Acme Corp")
      org.organization_memberships.create!(person: person)
      contact = @workspace.contacts.create!(email: "alice@acme.example", person: person)

      add_rule("Big Accounts", "organization", org.id)
      thread = create_thread(from: contact.email, contact: contact)

      expect(service.group_scope("Big Accounts").where(id: thread.id).exists?).to be(true)
    end

    it "does not include threads from contacts not in the organization" do
      person  = @workspace.people.create!(name: "Bob")
      org     = @workspace.organizations.create!(name: "Acme Corp")
      org.organization_memberships.create!(person: person)
      contact = @workspace.contacts.create!(email: "bob@acme.example", person: person)

      other_org = @workspace.organizations.create!(name: "Other Corp")
      add_rule("Big Accounts", "organization", other_org.id)
      thread = create_thread(from: contact.email, contact: contact)

      expect(service.group_scope("Big Accounts").where(id: thread.id).exists?).to be(false)
    end
  end

  # ---- document_type rule --------------------------------------------------

  describe "document_type rule" do
    it "includes threads linked to a document of the specified type" do
      dt     = @workspace.document_types.create!(name: "Invoice #{SecureRandom.hex(2)}", color: "#aabbcc")
      add_rule("Invoices", "document_type", dt.id)
      thread = create_thread(doc_type: dt)

      expect(service.group_scope("Invoices").where(id: thread.id).exists?).to be(true)
    end

    it "does not include threads without a matching document" do
      dt1 = @workspace.document_types.create!(name: "Invoice #{SecureRandom.hex(2)}", color: "#aabbcc")
      dt2 = @workspace.document_types.create!(name: "Receipt #{SecureRandom.hex(2)}", color: "#ccbbaa")
      add_rule("Invoices", "document_type", dt1.id)
      thread = create_thread(doc_type: dt2)

      expect(service.group_scope("Invoices").where(id: thread.id).exists?).to be(false)
    end

    it "does not include threads with no documents" do
      dt = @workspace.document_types.create!(name: "Invoice #{SecureRandom.hex(2)}", color: "#aabbcc")
      add_rule("Invoices", "document_type", dt.id)
      thread = create_thread

      expect(service.group_scope("Invoices").where(id: thread.id).exists?).to be(false)
    end
  end

  # ---- query rule ----------------------------------------------------------

  describe "query rule (structured filters only)" do
    it "matches threads by from: modifier" do
      addr = "newsletter-#{SecureRandom.hex(4)}@weekly.example"
      add_rule("Newsletters", "query", "from:#{addr}")
      matching = create_thread(from: addr)
      other    = create_thread  # random from address

      scope = service.group_scope("Newsletters")
      expect(scope.where(id: matching.id).exists?).to be(true)
      expect(scope.where(id: other.id).exists?).to be(false)
    end

    it "matches threads by is:unread filter" do
      add_rule("Unread", "query", "is:unread")
      unread = create_thread
      read   = create_thread
      read.email_messages.update_all(read: true)

      scope = service.group_scope("Unread")
      expect(scope.where(id: unread.id).exists?).to be(true)
      expect(scope.where(id: read.id).exists?).to be(false)
    end

    it "matches threads by has:attachment" do
      add_rule("With Attachments", "query", "has:attachment")
      with_att = create_thread
      with_att.email_messages.update_all(has_attachment: true)
      without  = create_thread

      scope = service.group_scope("With Attachments")
      expect(scope.where(id: with_att.id).exists?).to be(true)
      expect(scope.where(id: without.id).exists?).to be(false)
    end

    it "ignores free text and uses only filter modifiers" do
      # "important invoices" has a free-text word ("invoices") but no valid
      # modifier — the rule should be a no-op (nil from thread_id_subquery_for_rule
      # because parsed.filters? is false after stripping invalid modifiers).
      add_rule("Junk", "query", "invoices")
      thread = create_thread(subject: "my invoices")

      # group_scope returns nil (no applicable filters) → no threads matched
      # even though the subject contains "invoices".
      scope = service.group_scope("Junk")
      expect(scope).to be_nil
    end

    it "handles a query with both free text and a valid modifier" do
      add_rule("Bills", "query", "is:unread invoice stuff")
      unread = create_thread(subject: "invoice stuff")
      read   = create_thread(subject: "invoice stuff")
      read.email_messages.update_all(read: true)

      scope = service.group_scope("Bills")
      expect(scope).not_to be_nil
      expect(scope.where(id: unread.id).exists?).to be(true)
      expect(scope.where(id: read.id).exists?).to be(false)
    end
  end

  # ---- excluded_scope with rules -------------------------------------------

  describe "excluded_scope with rules" do
    it "includes rule-matched threads in excluded_scope" do
      addr = "billing-#{SecureRandom.hex(4)}@vendor.example"
      add_rule("Vendors", "sender", addr)
      thread = create_thread(from: addr)
      scope  = service.excluded_scope
      expect(scope).not_to be_nil
      expect(scope.where(id: thread.id).exists?).to be(true)
    end

    it "is nil when no grouped tags AND no rules exist" do
      bare_ws = Workspace.create!(name: "Bare WS #{SecureRandom.hex(4)}")
      bare_acc = EmailAccount.create!(
        workspace: bare_ws, email_address: "bare-#{SecureRandom.hex(4)}@example.com",
        provider: :google, refresh_token: "tok", active: true
      )
      svc = described_class.new(bare_ws, [ bare_acc.id ])
      expect(svc.excluded_scope).to be_nil
    end
  end

  # ---- guards apply to rule-matched threads --------------------------------

  describe "guards" do
    it "excludes replied threads from rule-based group_scope" do
      addr = "billing-#{SecureRandom.hex(4)}@vendor.example"
      add_rule("Vendors", "sender", addr)
      thread = create_thread(from: addr)
      thread.update!(last_outbound_at: Time.current)

      expect(service.group_scope("Vendors").where(id: thread.id).exists?).to be(false)
    end

    it "excludes starred-sender threads from rule-based group_scope" do
      email = "vip-#{SecureRandom.hex(4)}@vendor.example"
      contact = @workspace.contacts.create!(email: email, starred_at: Time.current)
      add_rule("Vendors", "sender", email)
      thread = create_thread(from: contact.email, contact: contact)

      expect(service.group_scope("Vendors").where(id: thread.id).exists?).to be(false)
    end

    it "excludes threads with important messages from rule-based group_scope" do
      addr = "billing-#{SecureRandom.hex(4)}@vendor.example"
      add_rule("Vendors", "sender", addr)
      thread = create_thread(from: addr, category: "important")

      expect(service.group_scope("Vendors").where(id: thread.id).exists?).to be(false)
    end
  end

  # ---- additive multi-group membership ------------------------------------

  describe "additive multi-group membership" do
    it "a thread matching a tag group AND a rule group appears in both" do
      addr = "billing-#{SecureRandom.hex(4)}@vendor.example"
      add_rule("Vendors", "sender", addr)
      thread = create_thread(from: addr, tags: [ @promo_tag ])

      promo_scope  = service.group_scope(@promo_tag.group_name)
      vendor_scope = service.group_scope("Vendors")
      expect(promo_scope.where(id: thread.id).exists?).to be(true)
      expect(vendor_scope.where(id: thread.id).exists?).to be(true)
    end

    it "build_groups includes both tag-based and rules-only groups" do
      addr = "billing-#{SecureRandom.hex(4)}@vendor.example"
      add_rule("Vendors", "sender", addr)
      create_thread(from: addr)
      create_thread(tags: [ @promo_tag ])

      groups = service.build_groups([ "INBOX" ])
      labels = groups.map { |g| g[:label] }
      expect(labels).to include(@promo_tag.group_name), "Expected promo tag group in build_groups"
      expect(labels).to include("Vendors"), "Expected rules-only Vendors group in build_groups"
    end

    it "a thread in a mixed group (tag + rule) is counted once per group" do
      addr = "billing-#{SecureRandom.hex(4)}@vendor.example"
      add_rule("Vendors", "sender", addr)
      # This thread triggers both the promo tag and the Vendors rule.
      create_thread(from: addr, tags: [ @promo_tag ])

      groups = service.build_groups([ "INBOX" ])
      vendor_row = groups.find { |g| g[:label] == "Vendors" }
      expect(vendor_row[:count]).to eq(1), "Thread counted once in Vendors group"
    end
  end

  # ---- rules-only group (no tags) -----------------------------------------

  describe "rules-only group" do
    it "group_scope returns threads matching the rule even without any tags" do
      addr = "noreply-#{SecureRandom.hex(4)}@service.example"
      add_rule("Auto-filed", "sender", addr)
      thread = create_thread(from: addr)

      scope = service.group_scope("Auto-filed")
      expect(scope).not_to be_nil
      expect(scope.where(id: thread.id).exists?).to be(true)
    end

    it "group_color returns nil for a rules-only group" do
      add_rule("Auto-filed", "sender", "noreply@service.example")
      expect(service.group_color("Auto-filed")).to be_nil
    end
  end
end
