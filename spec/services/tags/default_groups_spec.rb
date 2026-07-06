# frozen_string_literal: true

require "rails_helper"

RSpec.describe Tags::DefaultGroups do
  before do
    @workspace = Workspace.create!(name: "Default Groups WS #{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Ana", email_address: "ana-#{SecureRandom.hex(4)}@example.com", password: "password123"
    )
    @account = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    @account.email_account_users.create!(user: @user, owner: true, can_read: true, can_send: true)
  end

  # ── provision! ─────────────────────────────────────────────────────────────

  it "provision! creates exactly 4 default bucket tags" do
    expect { described_class.provision!(@workspace) }
      .to change { @workspace.tags.count }.by(4)
  end

  it "provision! creates tags covering all four bucket keys" do
    described_class.provision!(@workspace)
    buckets = @workspace.tags.where.not(default_bucket: nil).pluck(:default_bucket)
    expect(buckets.sort).to eq(described_class::BUCKETS.sort)
  end

  it "provision! sets correct group_name and name for promotions" do
    described_class.provision!(@workspace)
    tag = @workspace.tags.find_by!(default_bucket: "promotions")
    expect(tag.name).to eq("Newsletters & promos")
    expect(tag.group_name).to eq("Newsletters & promos")
  end

  it "provision! sets correct colors for each bucket" do
    described_class.provision!(@workspace)
    described_class::BUCKETS.each do |bucket|
      tag = @workspace.tags.find_by!(default_bucket: bucket)
      expected = described_class::COLORS.fetch(bucket)
      expect(tag.color).to eq(expected), "Wrong color for #{bucket}"
    end
  end

  it "provision! sets kind user, source local, hidden false on every tag" do
    described_class.provision!(@workspace)
    described_class::BUCKETS.each do |bucket|
      tag = @workspace.tags.find_by!(default_bucket: bucket)
      expect(tag.kind_user?).to be(true), "Expected kind=user for #{bucket}"
      expect(tag.local?).to be(true), "Expected source=local for #{bucket}"
      expect(tag.hidden).to eq(false), "Expected hidden=false for #{bucket}"
    end
  end

  it "provision! is idempotent -- second call creates no new tags" do
    described_class.provision!(@workspace)
    expect { described_class.provision!(@workspace) }
      .not_to change { @workspace.tags.count }
  end

  it "provision! preserves a user rename of an existing bucket tag" do
    described_class.provision!(@workspace)
    tag = @workspace.tags.find_by!(default_bucket: "promotions")
    tag.update!(name: "My Promos", group_name: "My Promos")

    described_class.provision!(@workspace)

    expect(tag.reload.name).to eq("My Promos"), "User rename must be preserved after re-provision"
  end

  # ── bucket_tag_for ──────────────────────────────────────────────────────────

  it "bucket_tag_for returns the tag for a known bucket" do
    described_class.provision!(@workspace)
    tag = described_class.bucket_tag_for(@workspace, "promotions")
    expect(tag).not_to be_nil
    expect(tag.default_bucket).to eq("promotions")
  end

  it "bucket_tag_for returns nil for the personal category" do
    described_class.provision!(@workspace)
    expect(described_class.bucket_tag_for(@workspace, "personal")).to be_nil
  end

  it "bucket_tag_for returns nil for important" do
    described_class.provision!(@workspace)
    expect(described_class.bucket_tag_for(@workspace, "important")).to be_nil
  end

  it "bucket_tag_for returns nil for unknown" do
    described_class.provision!(@workspace)
    expect(described_class.bucket_tag_for(@workspace, "unknown")).to be_nil
  end

  it "bucket_tag_for returns nil before provisioning" do
    expect(described_class.bucket_tag_for(@workspace, "promotions")).to be_nil
  end

  # ── tag_email! ─────────────────────────────────────────────────────────────

  it "tag_email! attaches the bucket tag for each of the four noise buckets" do
    described_class.provision!(@workspace)
    described_class::BUCKETS.each do |bucket|
      msg = build_message(category: bucket)
      described_class.tag_email!(msg)
      tag = described_class.bucket_tag_for(@workspace, bucket)
      expect(msg.email_message_tags.where(tag: tag).exists?)
        .to be(true), "Expected #{bucket} tag on message after tag_email!"
    end
  end

  it "tag_email! returns the tag for a noise bucket" do
    described_class.provision!(@workspace)
    msg = build_message(category: "promotions")
    result = described_class.tag_email!(msg)
    expect(result).to be_a(Tag)
    expect(result.default_bucket).to eq("promotions")
  end

  it "tag_email! returns nil and creates no tag for personal" do
    described_class.provision!(@workspace)
    msg = build_message(category: "personal")
    result = described_class.tag_email!(msg)
    expect(result).to be_nil
    expect(msg.email_message_tags.count).to eq(0)
  end

  it "tag_email! returns nil for important" do
    described_class.provision!(@workspace)
    msg = build_message(category: "important")
    expect(described_class.tag_email!(msg)).to be_nil
  end

  it "tag_email! returns nil for nil category" do
    described_class.provision!(@workspace)
    msg = build_message(category: nil)
    expect(described_class.tag_email!(msg)).to be_nil
  end

  it "tag_email! is idempotent -- second call never duplicates the tag" do
    described_class.provision!(@workspace)
    msg = build_message(category: "notifications")
    described_class.tag_email!(msg)
    expect { described_class.tag_email!(msg) }
      .not_to change { msg.email_message_tags.count }
  end

  it "tag_email! is additive -- works even when the message already has an unrelated tag" do
    described_class.provision!(@workspace)
    existing_tag = @workspace.tags.create!(name: "Finance #{SecureRandom.hex(4)}", color: "#aabbcc")
    msg = build_message(category: "social")
    msg.email_message_tags.create!(tag: existing_tag)

    described_class.tag_email!(msg)

    expect(msg.email_message_tags.count).to eq(2)
    social_tag = described_class.bucket_tag_for(@workspace, "social")
    expect(msg.email_message_tags.where(tag: social_tag).exists?)
      .to be(true), "Expected social bucket tag alongside the pre-existing tag"
  end

  it "tag_email! self-heals by provisioning when the bucket tag is missing" do
    # Workspace has no default groups yet -- tag_email! should provision and tag.
    msg = build_message(category: "updates")
    described_class.tag_email!(msg)

    updates_tag = described_class.bucket_tag_for(@workspace, "updates")
    expect(updates_tag).not_to be_nil, "provision! should have been triggered automatically"
    expect(msg.email_message_tags.where(tag: updates_tag).exists?)
      .to be(true), "Expected updates tag after self-heal"
  end

  private

  def build_message(category:)
    thread = @account.email_threads.create!(subject: "T #{SecureRandom.hex(4)}")
    @account.email_messages.create!(
      email_thread: thread,
      provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX",
      from_address: "sender@example.com",
      to_address: @account.email_address,
      subject: "Test",
      received_at: Time.current,
      read: false,
      has_attachment: false,
      category: category
    )
  end
end
