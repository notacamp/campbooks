require "rails_helper"

RSpec.describe EmailActions do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:message) { create(:email_message, email_account: account) }
  let!(:tag) { workspace.tags.create!(name: "invoice", color: "#2563eb") }

  before do
    create(:email_account_user, :viewer, user: user, email_account: account)
    Current.acting_user = user
    Current.workspace = workspace
  end

  after { Current.reset }

  describe ".run" do
    it "attaches an existing tag and returns its details" do
      out = described_class.run("add_tag", email_message: message, args: { tag_name: "invoice" }, user: user)

      expect(out[:success]).to be(true)
      expect(out.dig(:result, :tag, :name)).to eq("invoice")
      expect(out[:tool]).to eq("add_tag")
      expect(message.reload.tags).to include(tag)
    end

    it "fails with a specific message when the tag doesn't exist" do
      out = described_class.run("add_tag", email_message: message, args: { tag_name: "nope" }, user: user)

      expect(out[:success]).to be(false)
      expect(out[:message]).to match(/no tag named 'nope'/i)
    end

    it "blocks a tool on an account the user cannot read" do
      other = create(:email_message, email_account: create(:email_account, workspace: workspace))
      out = described_class.run("add_tag", email_message: other, args: { tag_name: "invoice" }, user: user)

      expect(out[:success]).to be(false)
      expect(out[:message]).to match(/access/i)
    end

    it "blocks a send-tool without send permission" do
      out = described_class.run("forward_email", email_message: message, args: { to_address: "x@example.com" }, user: user)

      expect(out[:success]).to be(false)
      expect(out[:message]).to match(/send/i)
    end

    it "returns a failure for an unknown tool" do
      out = described_class.run("not_a_tool", email_message: message, user: user)

      expect(out[:success]).to be(false)
      expect(out[:message]).to match(/unknown/i)
    end
  end

  describe "sender actions" do
    it "stars the sender (resolving the contact from the message)" do
      out = described_class.run("star_sender", email_message: message, user: user)

      expect(out[:success]).to be(true)
      expect(message.reload.contact).to be_present
      expect(message.contact.starred?).to be(true)
    end

    it "blocks the sender, clears any star, and enqueues the archive of existing mail" do
      message.contact = Contacts::Identifier.contact_for(message)
      message.contact.star!

      expect {
        out = described_class.run("block_sender", email_message: message, user: user)
        expect(out[:success]).to be(true)
      }.to have_enqueued_job(SenderBlockArchiveJob)

      contact = message.reload.contact
      expect(contact.blocked?).to be(true)
      expect(contact.starred?).to be(false)
    end

    it "allows the sender" do
      out = described_class.run("allow_sender", email_message: message, user: user)

      expect(out[:success]).to be(true)
      expect(message.reload.contact.allowed?).to be(true)
    end

    it "gates sender actions behind read access to the account" do
      other = create(:email_message, email_account: create(:email_account, workspace: workspace))
      out = described_class.run("block_sender", email_message: other, user: user)

      expect(out[:success]).to be(false)
      expect(out[:message]).to match(/access/i)
    end
  end

  describe "metadata" do
    it "builds value-dependent labels" do
      expect(described_class.definition("add_tag").label_for(tag_name: "invoice")).to eq("Tag: invoice")
      expect(described_class.definition("archive").label_for).to eq("Archive")
    end

    it "exposes the tools available on a surface" do
      expect(described_class.tools_for(:scout_suggest).map(&:id)).to include("add_tag", "archive", "forward_email")
    end

    it "exposes sender tools on the skim surface" do
      expect(described_class.tools_for(:skim).map(&:id)).to include("star_sender", "block_sender", "allow_sender")
    end
  end

  describe ".auto_safe?" do
    # Safe actions: non-destructive, perm: :read, listed on :scout_auto surface.
    %w[add_tag remove_tag archive trash snooze unsnooze reclassify bulk_archive bulk_tag].each do |tool|
      it "returns true for #{tool}" do
        expect(described_class.auto_safe?(tool)).to be(true)
      end
    end

    # Destructive action: destructive: true AND not on :scout_auto surface.
    it "returns false for block_sender (destructive: true)" do
      expect(described_class.auto_safe?("block_sender")).to be(false)
    end

    # Send-permission action: perm: :send, even though it lists :scout_auto in surfaces.
    it "returns false for forward_email (perm: :send)" do
      expect(described_class.auto_safe?("forward_email")).to be(false)
    end

    # Actions not listed on :scout_auto at all.
    it "returns false for create_calendar_event (not on :scout_auto surface)" do
      expect(described_class.auto_safe?("create_calendar_event")).to be(false)
    end

    it "returns false for star_sender (not on :scout_auto surface)" do
      expect(described_class.auto_safe?("star_sender")).to be(false)
    end

    # Unknown / attacker-supplied key.
    it "returns false for an unknown action key" do
      expect(described_class.auto_safe?("delete_everything")).to be(false)
    end
  end

  describe "create_calendar_event" do
    let(:calendar_account) { create(:calendar_account, workspace: workspace) }
    let(:calendar) { create(:calendar, calendar_account: calendar_account, is_writable: true, syncing: true) }

    before do
      create(:calendar_account_user, :editor, user: user, calendar_account: calendar_account)
    end

    context "when the user has a writable, syncing calendar" do
      before { calendar } # ensure it's created

      it "creates a CalendarEvent linked via source_email_message" do
        expect {
          out = described_class.run("create_calendar_event", email_message: message,
                                    args: { title: "Meeting", start_time: 1.day.from_now.iso8601 },
                                    user: user)
          expect(out[:success]).to be(true)
        }.to change(CalendarEvent, :count).by(1)

        event = CalendarEvent.last
        expect(event.source_email_message).to eq(message)
        expect(event.title).to eq("Meeting")
      end

      it "enqueues Calendars::EventWriteJob after creating the event" do
        expect {
          described_class.run("create_calendar_event", email_message: message,
                              args: { title: "Sync up", start_time: 1.day.from_now.iso8601 },
                              user: user)
        }.to have_enqueued_job(Calendars::EventWriteJob)
      end

      it "builds a value-dependent label from the title arg" do
        defn = described_class.definition("create_calendar_event")
        expect(defn.label_for(title: "Sprint demo")).to eq("Create event: Sprint demo")
        expect(defn.label_for(title: "")).to eq("Create calendar event")
      end
    end

    context "when the user has no writable calendar" do
      it "returns success: false with the no_calendar message" do
        # No calendar created — target_calendar returns nil
        out = described_class.run("create_calendar_event", email_message: message,
                                  args: { title: "Meeting" }, user: user)

        expect(out[:success]).to be(false)
        expect(out[:message]).to be_present
      end
    end
  end
end
