require "rails_helper"

RSpec.describe Feed::Rewind do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:now) { Time.utc(2026, 6, 22, 12, 0) }
  let(:starred_contact) { create(:contact, workspace: workspace, starred_at: now) }

  before do
    create(:email_account_user, user: user, email_account: account)
    # Fail open by default (no inbox-folder resolution) so these stay pure
    # signal-filter tests; the inbox-folder gate has its own block below.
    allow(Emails::InboxFolders).to receive(:ids_for).and_return([])
  end

  def email(received_at:, **attrs)
    create(:email_message, email_account: account, received_at: received_at, **attrs)
  end

  def rewind(**kw) = described_class.new(user, now: now, **kw)

  describe "the highlight filter" do
    it "keeps starred-sender, important, high-priority, attachment, and busy-thread mail" do
      starred   = email(contact: starred_contact, received_at: now - 1.day)
      important = email(category: "important", received_at: now - 2.days)
      high      = email(ai_priority: :high, received_at: now - 3.days)
      attach    = email(has_attachment: true, category: "personal", received_at: now - 4.days)

      thread = create(:email_thread, email_account: account)
      8.times { |i| email(email_thread: thread, category: "personal", received_at: now - (40 + i).days) }
      busy = thread.email_messages.order(:received_at).last

      ids = described_class.new(user, now: now).page.map(&:id) # PAGE_SIZE=8: 4 strong + 4 busy
      expect(ids).to include(starred.id, important.id, high.id, attach.id, busy.id)
    end

    it "drops noise: no-signal mail, bulk-category attachments, and newsletters" do
      email(category: "personal", received_at: now - 1.day)                       # no signal
      email(category: "promotions", has_attachment: true, received_at: now - 2.days) # bulk attachment
      email(category: "notifications", received_at: now - 3.days)                  # newsletter

      expect(rewind.page).to be_empty
    end
  end

  describe "the inbox-folder gate — archived mail drops out" do
    # The lockstep partner of Feed::Source#in_inbox / Skim: archiving moves a
    # message to the Archive folder (rewriting provider_folder_id), so an
    # inbox-only scope must drop it. Without this, archiving a "Looking back"
    # card removed it from view but it reappeared on the next scroll/reload.
    before { allow(Emails::InboxFolders).to receive(:ids_for).and_return([ "INBOX" ]) }

    it "keeps an inbox highlight but drops the same highlight once archived" do
      inbox    = email(contact: starred_contact, received_at: now - 1.day, provider_folder_id: "INBOX")
      archived = email(contact: starred_contact, received_at: now - 2.days, provider_folder_id: "ARCHIVE")

      ids = rewind.page.map(&:id)
      expect(ids).to include(inbox.id)
      expect(ids).not_to include(archived.id)
    end

    it "#any? is false once every past highlight has been archived" do
      email(contact: starred_contact, received_at: now - 1.day, provider_folder_id: "ARCHIVE")
      expect(rewind.any?).to be(false)
    end
  end

  describe "#entries — time chapters mixed with highlight cards" do
    it "opens a chapter (with a count) before each new period, card following" do
      email(contact: starred_contact, received_at: now - 2.days)        # June 2026 -> This month
      email(contact: starred_contact, received_at: Time.utc(2024, 3, 1)) # 2024

      entries = rewind.entries
      expect(entries.map { |e| e[:type] }).to eq(%i[chapter card chapter card])
      expect(entries.first[:label]).to eq(I18n.t("home.index.rewind_this_month"))
      expect(entries.first[:count]).to eq(1)
      expect(entries.third[:label]).to eq("2024")
    end

    it "does not repeat a chapter that continues from the previous page" do
      e1 = email(contact: starred_contact, received_at: Time.utc(2024, 5, 2))
      e2 = email(contact: starred_contact, received_at: Time.utc(2024, 5, 1))
      cursor = Feed::Rewind::Cursor.new(e1.received_at, e1.id, "2024")

      entries = described_class.new(user, now: now, before: cursor).entries
      expect(entries.map { |e| e[:type] }).to all(eq(:card)) # still in 2024, no new chapter
      expect(entries.map { |e| e[:email].id }).to eq([ e2.id ])
    end

    it "labels each card with its dominant reason, strong signals first" do
      email(contact: starred_contact, has_attachment: true, received_at: now - 1.day) # starred beats attachment

      card = rewind.entries.find { |e| e[:type] == :card }
      expect(card[:reason]).to eq(:starred)
    end
  end

  describe "keyset pagination" do
    it "walks every highlight newest -> oldest, once each" do
      mail = Array.new(20) { |i| email(contact: starred_contact, received_at: now - (i + 1).days) }

      seen, cursor = [], nil
      10.times do
        r = described_class.new(user, now: now, before: cursor)
        seen.concat(r.page)
        cursor = r.next_cursor
        break if cursor.nil?
      end

      expect(seen.map(&:id)).to eq(mail.map(&:id))
    end
  end

  describe "#any?, dedup, permissions" do
    it "excludes highlights already shown as a curated card" do
      shown = email(category: "important", ai_action_prompt: "Reply", received_at: now - 1.hour)
      other = email(category: "important", received_at: now - 30.days)
      Feed::Generator.for_user(user) # materializes a curated card for `shown`

      expect(rewind.page.map(&:id)).to eq([ other.id ])
    end

    it "is permission-scoped to the user's readable accounts" do
      other_account = create(:email_account, workspace: workspace)
      hidden = create(:email_message, email_account: other_account, contact: starred_contact, received_at: now - 1.day)

      expect(rewind.page.map(&:id)).not_to include(hidden.id)
    end

    it "#any? is false when the mailbox holds no highlights" do
      email(category: "personal", received_at: now - 1.day)
      expect(rewind.any?).to be(false)
    end
  end

  describe ".cursor_from_params" do
    it "round-trips before / before_id / period" do
      at = Time.zone.parse("2024-03-02T01:02:03.000000Z")
      cur = described_class.cursor_from_params(before: at.iso8601(6), before_id: "7", period: "2024")

      expect(cur.sort_at).to be_within(0.001).of(at)
      expect([ cur.id, cur.period ]).to eq([ "7", "2024" ])
    end

    it "returns nil on missing or malformed params" do
      expect(described_class.cursor_from_params({})).to be_nil
      expect(described_class.cursor_from_params(before: "not-a-date", before_id: "1")).to be_nil
    end
  end
end
