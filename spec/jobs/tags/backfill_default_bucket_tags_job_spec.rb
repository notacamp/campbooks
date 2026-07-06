# frozen_string_literal: true

require "rails_helper"

RSpec.describe Tags::BackfillDefaultBucketTagsJob, type: :job do
  before do
    @workspace = Workspace.create!(name: "Backfill Bucket Tags WS #{SecureRandom.hex(4)}")
    @account = EmailAccount.create!(
      workspace: @workspace, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    Tags::DefaultGroups.provision!(@workspace)
    @promo_tag = Tags::DefaultGroups.bucket_tag_for(@workspace, "promotions")
  end

  def build_message(category:, account: @account)
    thread = account.email_threads.create!(subject: "T #{SecureRandom.hex(4)}")
    account.email_messages.create!(
      email_thread: thread,
      provider_message_id: "m-#{SecureRandom.hex(4)}",
      provider_folder_id: "INBOX",
      from_address: "sender@example.com",
      to_address: account.email_address,
      subject: "Test",
      received_at: Time.current,
      read: false,
      has_attachment: false,
      category: category
    )
  end

  it "attaches the bucket tag to an existing message with a matching category" do
    msg = build_message(category: "promotions")
    described_class.perform_now
    expect(msg.email_message_tags.where(tag_id: @promo_tag.id).exists?)
      .to be(true), "Expected promotions tag after backfill"
  end

  it "is idempotent -- re-running does not create duplicate tags" do
    msg = build_message(category: "promotions")
    described_class.perform_now
    count_after_first = msg.email_message_tags.count

    expect { described_class.perform_now }
      .not_to change { msg.email_message_tags.count }
    expect(msg.email_message_tags.count).to eq(count_after_first)
  end

  it "skips messages with a non-noise category (personal)" do
    msg = build_message(category: "personal")
    described_class.perform_now
    expect(msg.email_message_tags.count).to eq(0)
  end

  it "skips messages with nil category" do
    msg = build_message(category: nil)
    described_class.perform_now
    expect(msg.email_message_tags.count).to eq(0)
  end

  it "attaches correct bucket tags for all four buckets" do
    msgs = Tags::DefaultGroups::BUCKETS.map { |b| [ b, build_message(category: b) ] }.to_h

    described_class.perform_now

    Tags::DefaultGroups::BUCKETS.each do |bucket|
      tag = Tags::DefaultGroups.bucket_tag_for(@workspace, bucket)
      expect(msgs[bucket].email_message_tags.where(tag_id: tag.id).exists?)
        .to be(true), "Expected #{bucket} tag after backfill"
    end
  end

  it "provisions default groups for a workspace that has none, then tags the messages" do
    bare_ws = Workspace.create!(name: "Bare Provision WS #{SecureRandom.hex(4)}")
    bare_account = EmailAccount.create!(
      workspace: bare_ws, email_address: "bare-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    msg = build_message(category: "promotions", account: bare_account)

    described_class.perform_now

    promo_tag = Tags::DefaultGroups.bucket_tag_for(bare_ws, "promotions")
    expect(promo_tag).not_to be_nil, "provision! must have been called for the bare workspace"
    expect(msg.email_message_tags.where(tag_id: promo_tag.id).exists?)
      .to be(true), "Expected promotions tag attached to the bare workspace's message"
  end
end
