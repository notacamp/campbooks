# frozen_string_literal: true

require "rails_helper"

# Minimal stand-in for a Net::IMAP::FetchData response.
# Exposes `.attr` just like the real object so normalization code is exercised
# without any network contact.
FetchDataDouble = Struct.new(:attr, keyword_init: false)

RSpec.describe Imap::MailClient, type: :service do
  let(:account) { create(:email_account, :imap) }
  let(:imap)    { instance_double(Net::IMAP) }
  let(:client)  { described_class.new(account) }

  before do
    # Suppress the host guard so unit tests don't need DNS.
    allow(Imap::HostGuard).to receive(:validate!)

    # Wire up the IMAP connection double.
    allow(Net::IMAP).to receive(:new).and_return(imap)
    allow(imap).to receive(:starttls)
    allow(imap).to receive(:auth_capable?).and_return(false)
    allow(imap).to receive(:capable?).and_return(false)
    allow(imap).to receive(:capability).and_return([ "IMAP4rev1" ])
    allow(imap).to receive(:login)
    allow(imap).to receive(:logout)
    allow(imap).to receive(:disconnect)
  end

  # --- Helper to build a minimal FETCH attr hash ---

  def build_envelope(subject: "Hello", from_name: "Alice", from_addr: "alice@example.com",
                     to_addr: "bob@example.com", date: "Mon, 01 Jan 2024 12:00:00 +0000")
    from = instance_double(
      Net::IMAP::Address, name: from_name, mailbox: from_addr.split("@").first,
                          host: from_addr.split("@").last, route: nil
    )
    to_addr_parts = to_addr.split("@")
    to_a = instance_double(
      Net::IMAP::Address, name: nil, mailbox: to_addr_parts.first,
                          host: to_addr_parts.last, route: nil
    )
    instance_double(
      Net::IMAP::Envelope,
      subject: subject, from: [ from ], to: [ to_a ], date: date,
      cc: nil, bcc: nil, reply_to: nil, in_reply_to: nil, message_id: nil
    )
  end

  def build_body_structure(has_attachment: false)
    text_plain = instance_double(
      Net::IMAP::BodyTypeText,
      media_type: "TEXT", subtype: "PLAIN", param: {}, content_id: nil,
      disposition: nil
    )
    if has_attachment
      dsp = instance_double(Net::IMAP::ContentDisposition, dsp_type: "ATTACHMENT",
                            param: { "FILENAME" => "file.pdf" })
      attachment = instance_double(
        Net::IMAP::BodyTypeBasic,
        media_type: "APPLICATION", subtype: "PDF", param: { "NAME" => "file.pdf" },
        content_id: nil, disposition: dsp
      )
      instance_double(
        Net::IMAP::BodyTypeMultipart,
        media_type: "MULTIPART", subtype: "MIXED", param: {},
        disposition: nil,
        parts: [ text_plain, attachment ]
      )
    else
      text_plain
    end
  end

  def build_fetch_data(uid:, message_id: "<msg001@example.com>", subject: "Hello",
                       flags: [], internal_date: "01-Jan-2024 12:00:00 +0000",
                       has_attachment: false)
    header_raw = "Message-ID: #{message_id}\r\nX-Campbooks-Kind: test\r\n\r\n"
    FetchDataDouble.new(
      "UID"          => uid,
      "ENVELOPE"     => build_envelope(subject: subject),
      "FLAGS"        => flags,
      "INTERNALDATE" => internal_date,
      "BODYSTRUCTURE" => build_body_structure(has_attachment: has_attachment),
      "BODY[HEADER.FIELDS (MESSAGE-ID LIST-UNSUBSCRIBE PRECEDENCE AUTO-SUBMITTED X-CAMPBOOKS-KIND)]" =>
        header_raw
    )
  end

  # ─────────────────────────────────────────────
  # Auth error mapping
  # ─────────────────────────────────────────────

  describe "auth error mapping" do
    context "when the server returns AUTHENTICATIONFAILED" do
      before do
        code = instance_double(Net::IMAP::ResponseCode, name: "AUTHENTICATIONFAILED")
        text = instance_double(Net::IMAP::ResponseText, code: code, text: "Authentication failed")
        resp = instance_double(Net::IMAP::TaggedResponse, data: text)
        err  = Net::IMAP::NoResponseError.new(resp)
        allow(imap).to receive(:login).and_raise(err)
      end

      it "raises PermanentAuthError" do
        expect { client.send(:open_imap_connection) }.to raise_error(PermanentAuthError)
      end

      it "does not include the password in the error message" do
        begin
          client.send(:open_imap_connection)
        rescue PermanentAuthError => e
          expect(e.message).not_to include(account.imap_password)
        end
      end
    end

    context "when the server returns a different NO response" do
      before do
        code = instance_double(Net::IMAP::ResponseCode, name: "UNAVAILABLE")
        text = instance_double(Net::IMAP::ResponseText, code: code, text: "Service unavailable")
        resp = instance_double(Net::IMAP::TaggedResponse, data: text)
        err  = Net::IMAP::NoResponseError.new(resp)
        allow(imap).to receive(:login).and_raise(err)
      end

      it "raises AuthenticationError (not PermanentAuthError)" do
        expect { client.send(:open_imap_connection) }.to raise_error(AuthenticationError)
        expect { client.send(:open_imap_connection) }.not_to raise_error(PermanentAuthError)
      end
    end
  end

  # ─────────────────────────────────────────────
  # Folder listing
  # ─────────────────────────────────────────────

  describe "#list_folders" do
    def make_mbox(name, attrs, delim = "/")
      instance_double(Net::IMAP::MailboxList, name: name, attr: attrs, delim: delim)
    end

    before do
      allow(imap).to receive(:list).and_return([
        make_mbox("INBOX",                  []),
        make_mbox("Sent Messages",          [ :Sent ]),
        make_mbox("Deleted Messages",       [ :Trash ]),
        make_mbox("[Gmail]/Drafts",         [ :Drafts ]),
        make_mbox("[Gmail]/Spam",           [ :Junk ]),
        make_mbox("[Gmail]/Archive",        [ :Archive ]),
        make_mbox("[Gmail]/All Mail",       [ :All ]),         # should be skipped
        make_mbox("INBOX.noselect.folder",  [ :Noselect ]),   # should be skipped
        make_mbox("Flagged",                [ :Flagged ]),     # should be skipped
        make_mbox("Work/Receipts",          [])
      ])
    end

    it "maps INBOX (any case) to the canonical display name Inbox" do
      folder = client.list_folders.find { |f| f["folderId"] == "INBOX" }
      expect(folder["folderName"]).to eq("Inbox")
    end

    it "maps a :Sent special-use attr to Sent" do
      folder = client.list_folders.find { |f| f["folderId"] == "Sent Messages" }
      expect(folder["folderName"]).to eq("Sent")
    end

    it "maps a :Trash special-use attr to Trash" do
      folder = client.list_folders.find { |f| f["folderId"] == "Deleted Messages" }
      expect(folder["folderName"]).to eq("Trash")
    end

    it "maps :Drafts, :Junk, :Archive special-use attrs correctly" do
      names = client.list_folders.map { |f| f["folderName"] }
      expect(names).to include("Drafts", "Spam", "Archive")
    end

    it "skips :Noselect, :All, and :Flagged entries" do
      names = client.list_folders.map { |f| f["folderName"] }
      expect(names).not_to include("All Mail", "Flagged")
      expect(client.list_folders.none? { |f| f["folderId"] == "INBOX.noselect.folder" }).to be(true)
    end

    it "falls back to full decoded name for unknown folders" do
      folder = client.list_folders.find { |f| f["folderId"] == "Work/Receipts" }
      expect(folder).not_to be_nil
      expect(folder["folderName"]).to eq("Work/Receipts")
    end

    context "when a folder name matches a known pattern (no special-use attr)" do
      before do
        allow(imap).to receive(:list).and_return([
          make_mbox("Deleted Messages", [])  # no special-use attr, name matches Trash pattern
        ])
      end

      it "canonicalizes Deleted Messages to Trash via name matching" do
        folder = client.list_folders.find { |f| f["folderId"] == "Deleted Messages" }
        expect(folder["folderName"]).to eq("Trash")
      end
    end
  end

  # ─────────────────────────────────────────────
  # normalize_message
  # ─────────────────────────────────────────────

  describe "normalized message hash" do
    let(:fetch_data) { build_fetch_data(uid: 42, message_id: "<abc123@mail.example.com>") }

    it "strips angle brackets from Message-ID" do
      msg = client.send(:normalize_message, fetch_data, "INBOX")
      expect(msg["messageId"]).to eq("abc123@mail.example.com")
    end

    it "sets folderId to the raw folder name" do
      msg = client.send(:normalize_message, fetch_data, "INBOX")
      expect(msg["folderId"]).to eq("INBOX")
    end

    it "sets summary to nil" do
      msg = client.send(:normalize_message, fetch_data, "INBOX")
      expect(msg["summary"]).to be_nil
    end

    it "sets providerThreadId to nil" do
      msg = client.send(:normalize_message, fetch_data, "INBOX")
      expect(msg["providerThreadId"]).to be_nil
    end

    it "encodes receivedTime as epoch milliseconds integer" do
      msg = client.send(:normalize_message, fetch_data, "INBOX")
      expect(msg["receivedTime"]).to be_a(Integer)
      expect(msg["receivedTime"]).to be > 0
    end

    it "returns status '0' when :Seen flag is absent" do
      msg = client.send(:normalize_message, fetch_data, "INBOX")
      expect(msg["status"]).to eq("0")
    end

    it "returns status '1' when :Seen flag is present" do
      fd  = build_fetch_data(uid: 42, flags: [ :Seen ])
      msg = client.send(:normalize_message, fd, "INBOX")
      expect(msg["status"]).to eq("1")
    end

    it "returns hasAttachment as the string '0' when no attachment" do
      msg = client.send(:normalize_message, build_fetch_data(uid: 42), "INBOX")
      expect(msg["hasAttachment"]).to eq("0")
    end

    it "returns hasAttachment as the string '1' when an attachment is present" do
      fd  = build_fetch_data(uid: 42, has_attachment: true)
      msg = client.send(:normalize_message, fd, "INBOX")
      expect(msg["hasAttachment"]).to eq("1")
    end

    it "passes through X-Campbooks-Kind as header_campbooks_kind" do
      msg = client.send(:normalize_message, fetch_data, "INBOX")
      expect(msg["header_campbooks_kind"]).to eq("test")
    end

    it "does not include a providerLabels key" do
      msg = client.send(:normalize_message, fetch_data, "INBOX")
      expect(msg).not_to have_key("providerLabels")
    end

    context "when Message-ID header is absent" do
      let(:fetch_data) do
        fd = build_fetch_data(uid: 42)
        # Remove the Message-ID from the raw header value
        fd.attr["BODY[HEADER.FIELDS (MESSAGE-ID LIST-UNSUBSCRIBE PRECEDENCE AUTO-SUBMITTED X-CAMPBOOKS-KIND)]"] =
          "X-Campbooks-Kind: test\r\n\r\n"
        fd
      end

      it "builds a noid- fallback id" do
        msg = client.send(:normalize_message, fetch_data, "INBOX")
        expect(msg["messageId"]).to start_with("noid-")
        expect(msg["messageId"].length).to eq(5 + 40)  # "noid-" + 40 hex chars
      end
    end
  end

  # ─────────────────────────────────────────────
  # fetch_new_messages — n:* echo filter
  # ─────────────────────────────────────────────

  describe "#fetch_new_messages" do
    before do
      allow(imap).to receive(:examine)
    end

    it "filters out messages whose UID is below from_uid (n:* echo)" do
      # The server ALWAYS returns at least the highest-UID message for n:* even
      # when nothing is new. Simulate that by returning a UID below from_uid.
      stale = build_fetch_data(uid: 99, message_id: "<stale@example.com>")
      allow(imap).to receive(:uid_fetch).and_return([ stale ])

      result = client.fetch_new_messages("INBOX", from_uid: 100)
      expect(result).to be_empty
    end

    it "includes messages at or above from_uid" do
      new_msg = build_fetch_data(uid: 100, message_id: "<new@example.com>")
      allow(imap).to receive(:uid_fetch).and_return([ new_msg ])

      result = client.fetch_new_messages("INBOX", from_uid: 100)
      expect(result.size).to eq(1)
      expect(result.first["messageId"]).to eq("new@example.com")
    end
  end

  # ─────────────────────────────────────────────
  # Content extraction
  # ─────────────────────────────────────────────

  describe "#get_message_content" do
    let(:raw_multipart) do
      m = Mail.new
      m.from    = "sender@example.com"
      m.to      = "rcpt@example.com"
      m.subject = "Test"
      m.message_id = "<fixture@example.com>"
      m.html_part { content_type "text/html; charset=UTF-8"; self.body = "<p>Hello HTML</p>" }
      m.text_part { content_type "text/plain; charset=UTF-8"; self.body = "Hello text" }
      m.to_s
    end

    before do
      allow(imap).to receive(:examine)
      allow(imap).to receive(:uid_search).and_return([ 77 ])
      allow(imap).to receive(:uid_fetch).with(77, "BODY.PEEK[]").and_return(
        [ FetchDataDouble.new("BODY[]" => raw_multipart) ]
      )
    end

    it "returns the HTML part when present" do
      result = client.get_message_content("fixture@example.com", "INBOX")
      expect(result).to include("<p>Hello HTML</p>")
    end

    context "when there is no HTML part (text/plain only)" do
      let(:raw_textonly) do
        m = Mail.new
        m.content_type = "text/plain; charset=UTF-8"
        m.body = "Plain text body & special <chars>"
        m.to_s
      end

      before do
        allow(imap).to receive(:uid_fetch).with(77, "BODY.PEEK[]").and_return(
          [ FetchDataDouble.new("BODY[]" => raw_textonly) ]
        )
      end

      it "wraps text/plain in a pre-wrap div with HTML-escaped content" do
        result = client.get_message_content("fixture@example.com", "INBOX")
        expect(result).to include("white-space: pre-wrap")
        expect(result).to include("&amp;")
        expect(result).to include("&lt;chars&gt;")
      end
    end
  end

  # ─────────────────────────────────────────────
  # Attachments & inline images
  # ─────────────────────────────────────────────

  describe "attachment handling" do
    let(:raw_multipart_with_attachments) do
      m = Mail.new
      m.from    = "a@example.com"
      m.to      = "b@example.com"
      m.subject = "Attach test"
      m.message_id = "<attach-fixture@example.com>"
      m.html_part { content_type "text/html; charset=UTF-8"; self.body = "<p>body</p>" }
      # Real (non-inline) attachment
      m.attachments["report.pdf"] = { content: "PDF data", mime_type: "application/pdf" }
      # Inline image — the mail gem auto-generates Content-ID when encoding, so we
      # set content_id and content_disposition directly on the part AFTER adding it.
      m.attachments["photo.jpg"] = { content: "JPEG data", mime_type: "image/jpeg" }
      photo = m.attachments.find { |a| a.filename == "photo.jpg" }
      photo.content_id         = "<img001@example.com>"
      photo.content_disposition = "inline; filename=photo.jpg"
      m.to_s
    end

    before do
      allow(imap).to receive(:examine)
      allow(imap).to receive(:uid_search).and_return([ 55 ])
      allow(imap).to receive(:uid_fetch).with(55, "BODY.PEEK[]").and_return(
        [ FetchDataDouble.new("BODY[]" => raw_multipart_with_attachments) ]
      )
    end

    describe "#list_message_attachments" do
      subject(:atts) { client.list_message_attachments("attach-fixture@example.com", "INBOX") }

      it "lists real attachments and inline images" do
        # html_part is neither attachment? nor has content_id → excluded
        expect(atts.map { |a| a["fileName"] }).to include("report.pdf", "photo.jpg")
      end

      it "sets attachmentType nil for real attachments" do
        pdf = atts.find { |a| a["fileName"] == "report.pdf" }
        expect(pdf["attachmentType"]).to be_nil
      end

      it "sets attachmentType 'inline' for inline images" do
        img = atts.find { |a| a["fileName"] == "photo.jpg" }
        expect(img["attachmentType"]).to eq("inline")
      end

      it "strips angle brackets from contentId" do
        img = atts.find { |a| a["fileName"] == "photo.jpg" }
        expect(img["contentId"]).to eq("img001@example.com")
      end

      it "uses the index in all_parts as attachmentId (string)" do
        atts.each { |a| expect(a["attachmentId"]).to match(/\A\d+\z/) }
      end
    end

    describe "#download_attachment" do
      it "returns the decoded content for the part at the given index" do
        atts  = client.list_message_attachments("attach-fixture@example.com", "INBOX")
        pdf   = atts.find { |a| a["fileName"] == "report.pdf" }
        data  = client.download_attachment("attach-fixture@example.com", "INBOX", pdf["attachmentId"])
        expect(data).to eq("PDF data")
      end
    end

    describe "#download_inline_image" do
      it "returns the decoded content for the part with the matching content_id" do
        data = client.download_inline_image("attach-fixture@example.com", "INBOX", "img001@example.com")
        expect(data).to eq("JPEG data")
      end

      it "also matches when the caller passes the bracketed form" do
        data = client.download_inline_image("attach-fixture@example.com", "INBOX", "<img001@example.com>")
        expect(data).to eq("JPEG data")
      end

      it "returns nil when no part matches the content_id" do
        data = client.download_inline_image("attach-fixture@example.com", "INBOX", "nonexistent")
        expect(data).to be_nil
      end
    end
  end

  # ─────────────────────────────────────────────
  # mark_read groups by folder and calls uid_store
  # ─────────────────────────────────────────────

  describe "#mark_read" do
    let!(:msg1) do
      create(:email_message, email_account: account,
             provider_message_id: "mid1", provider_folder_id: "INBOX")
    end
    let!(:msg2) do
      create(:email_message, email_account: account,
             provider_message_id: "mid2", provider_folder_id: "INBOX")
    end
    let!(:msg3) do
      create(:email_message, email_account: account,
             provider_message_id: "mid3", provider_folder_id: "Sent")
    end

    before do
      allow(imap).to receive(:select)
      allow(imap).to receive(:uid_search).with([ "HEADER", "Message-ID", "mid1" ]).and_return([ 10 ])
      allow(imap).to receive(:uid_search).with([ "HEADER", "Message-ID", "mid2" ]).and_return([ 11 ])
      allow(imap).to receive(:uid_search).with([ "HEADER", "Message-ID", "mid3" ]).and_return([ 20 ])
      allow(imap).to receive(:uid_store)
    end

    it "groups ids by their DB folder and calls uid_store once per folder" do
      client.mark_read(%w[mid1 mid2 mid3])
      # INBOX group: UIDs 10 and 11; Sent group: UID 20
      expect(imap).to have_received(:uid_store).twice
    end

    it "returns true" do
      expect(client.mark_read(%w[mid1])).to be(true)
    end

    it "skips ids with no matching DB row silently" do
      expect { client.mark_read(%w[unknown-id]) }.not_to raise_error
    end
  end

  # ─────────────────────────────────────────────
  # move_to_folder — MOVE vs copy+delete+expunge
  # ─────────────────────────────────────────────

  describe "#move_to_folder" do
    let!(:msg) do
      create(:email_message, email_account: account,
             provider_message_id: "move-mid", provider_folder_id: "INBOX")
    end

    before do
      allow(imap).to receive(:select)
      allow(imap).to receive(:uid_search).with([ "HEADER", "Message-ID", "move-mid" ]).and_return([ 5 ])
    end

    context "when the MOVE capability is present" do
      before { allow(imap).to receive(:capable?).with("MOVE").and_return(true) }

      it "uses uid_move" do
        allow(imap).to receive(:uid_move)
        client.move_to_folder([ "move-mid" ], "Archive")
        expect(imap).to have_received(:uid_move).with([ 5 ], "Archive")
      end
    end

    context "when the MOVE capability is absent" do
      before do
        allow(imap).to receive(:capable?).with("MOVE").and_return(false)
        allow(imap).to receive(:capable?).with("UIDPLUS").and_return(true)
        allow(imap).to receive(:uid_copy)
        allow(imap).to receive(:uid_store)
        allow(imap).to receive(:uid_expunge)
      end

      it "copies, flags Deleted, then uid_expunge" do
        client.move_to_folder([ "move-mid" ], "Archive")
        expect(imap).to have_received(:uid_copy).with([ 5 ], "Archive")
        expect(imap).to have_received(:uid_store).with([ 5 ], "+FLAGS", [ :Deleted ])
        expect(imap).to have_received(:uid_expunge).with([ 5 ])
      end
    end

    context "when neither MOVE nor UIDPLUS is present" do
      before do
        allow(imap).to receive(:capable?).and_return(false)
        allow(imap).to receive(:uid_copy)
        allow(imap).to receive(:uid_store)
        allow(imap).to receive(:expunge)
      end

      it "falls back to plain expunge" do
        client.move_to_folder([ "move-mid" ], "Archive")
        expect(imap).to have_received(:expunge)
      end
    end
  end

  # ─────────────────────────────────────────────
  # Outbound: send_message
  # ─────────────────────────────────────────────

  describe "#send_message" do
    let(:smtp) { instance_double(Net::SMTP) }

    before do
      allow(Net::SMTP).to receive(:new).and_return(smtp)
      allow(smtp).to receive(:enable_tls)
      allow(smtp).to receive(:enable_starttls)
      allow(smtp).to receive(:open_timeout=)
      allow(smtp).to receive(:start).and_yield(smtp)
      allow(smtp).to receive(:send_message)
      allow(imap).to receive(:list).and_return([])  # no Sent folder → skip APPEND
    end

    it "calls smtp.start with :plain auth and the sender's domain as HELO" do
      client.send_message(subject: "Hi", body: "<p>Hi</p>", to_address: "b@example.com")
      helo = account.email_address.split("@").last
      expect(smtp).to have_received(:start).with(helo, account.imap_login_username,
                                                  account.imap_password, :plain)
    end

    it "enables starttls when smtp_security is starttls" do
      client.send_message(subject: "Hi", body: "<p>Hi</p>", to_address: "b@example.com")
      expect(smtp).to have_received(:enable_starttls)
    end

    it "returns a hash with a messageId key" do
      result = client.send_message(subject: "Hi", body: "<p>Hi</p>", to_address: "b@example.com")
      expect(result).to be_a(Hash)
      expect(result["messageId"]).to be_a(String).and(be_present)
    end

    it "appends to the Sent folder with :Seen when the folder exists" do
      allow(imap).to receive(:list).and_return([
        instance_double(Net::IMAP::MailboxList, name: "Sent", attr: [], delim: "/")
      ])
      allow(imap).to receive(:append)

      client.send_message(subject: "Hi", body: "<p>Hi</p>", to_address: "b@example.com")
      expect(imap).to have_received(:append).with("Sent", kind_of(String), [ :Seen ])
    end

    context "when attachments are provided" do
      it "accepts both symbol and string keys in the attachment hash" do
        expect do
          client.send_message(
            subject: "With attach", body: "<p>x</p>", to_address: "b@example.com",
            attachments: [
              { filename: "a.pdf", content_type: "application/pdf", data: "PDF" },
              { "filename" => "b.pdf", "content_type" => "application/pdf", "data" => "PDF2" }
            ]
          )
        end.not_to raise_error
      end
    end

    context "when SMTP auth fails with :plain then succeeds with :login" do
      before do
        call_count = 0
        allow(smtp).to receive(:start) do |*args, &block|
          call_count += 1
          if call_count == 1
            raise Net::SMTPAuthenticationError.new("AUTH PLAIN failed")
          else
            block.call(smtp)
          end
        end
      end

      it "retries with :login and does not raise" do
        expect do
          client.send_message(subject: "x", body: "x", to_address: "b@example.com")
        end.not_to raise_error
      end
    end

    context "when SMTP auth fails with both :plain and :login" do
      before do
        allow(smtp).to receive(:start).and_raise(Net::SMTPAuthenticationError.new("AUTH failed"))
      end

      it "raises PermanentAuthError" do
        expect do
          client.send_message(subject: "x", body: "x", to_address: "b@example.com")
        end.to raise_error(PermanentAuthError)
      end
    end
  end

  # ─────────────────────────────────────────────
  # Outbound: save_draft / send_draft round-trip
  # ─────────────────────────────────────────────

  describe "draft round-trip" do
    let(:smtp) { instance_double(Net::SMTP) }
    let(:draft_uid) { 99 }

    before do
      # Set up a Drafts folder.
      allow(imap).to receive(:list).and_return([
        instance_double(Net::IMAP::MailboxList, name: "Drafts", attr: [ :Drafts ], delim: "/")
      ])
      allow(imap).to receive(:append).and_return(nil)

      allow(Net::SMTP).to receive(:new).and_return(smtp)
      allow(smtp).to receive(:enable_starttls)
      allow(smtp).to receive(:open_timeout=)
      allow(smtp).to receive(:start).and_yield(smtp)
      allow(smtp).to receive(:send_message)
    end

    describe "#save_draft" do
      it "returns a hash with a non-blank messageId" do
        result = client.save_draft(subject: "Draft", body: "<p>draft</p>")
        expect(result["messageId"]).to be_a(String).and(be_present)
      end

      it "appends to Drafts with :Draft and :Seen flags" do
        client.save_draft(subject: "Draft", body: "<p>draft</p>")
        expect(imap).to have_received(:append).with("Drafts", kind_of(String), [ :Draft, :Seen ])
      end
    end

    describe "#send_draft" do
      let(:draft_message_id) { "draft-mid@example.com" }
      let(:draft_raw) do
        m = Mail.new
        m.from    = account.email_address
        m.to      = "dest@example.com"
        m.subject = "My draft"
        m.message_id = "<#{draft_message_id}>"
        m.content_type = "text/html; charset=UTF-8"
        m.body = "<p>draft content</p>"
        m.to_s
      end

      before do
        allow(imap).to receive(:examine)
        allow(imap).to receive(:select)
        allow(imap).to receive(:uid_search)
          .with([ "HEADER", "Message-ID", draft_message_id ])
          .and_return([ draft_uid ])
        allow(imap).to receive(:uid_fetch)
          .with(draft_uid, "BODY.PEEK[]")
          .and_return([ FetchDataDouble.new("BODY[]" => draft_raw) ])
        allow(imap).to receive(:uid_store)
        allow(imap).to receive(:expunge)
      end

      it "SMTP-sends the draft" do
        client.send_draft(draft_message_id)
        expect(smtp).to have_received(:send_message)
      end

      it "deletes the draft after sending" do
        client.send_draft(draft_message_id)
        expect(imap).to have_received(:uid_store).with(draft_uid, "+FLAGS", [ :Deleted ])
        expect(imap).to have_received(:expunge)
      end

      it "returns the original draft_message_id" do
        result = client.send_draft(draft_message_id)
        expect(result["messageId"]).to eq(draft_message_id)
      end
    end
  end
end
