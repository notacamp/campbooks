# frozen_string_literal: true

require "rails_helper"

RSpec.describe UserSyncChannel, type: :channel do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before { stub_connection(current_user: user) }

  it "confirms the subscription and streams for the connected user" do
    subscribe

    expect(subscription).to be_confirmed
    expect(subscription).to have_stream_for(user)
  end

  it "publishes a JSON envelope to the user's stream" do
    expect {
      UserSyncChannel.publish(user, topic: "people", action: "upsert",
                              id: "row-1", payload: { name: "Ada" })
    }.to have_broadcasted_to(user).from_channel(described_class).with(
      hash_including(topic: "people", action: "upsert", id: "row-1")
    )
  end

  it "is a no-op for a nil user" do
    expect { UserSyncChannel.publish(nil, topic: "people", action: "refresh") }
      .not_to raise_error
  end
end
