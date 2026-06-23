require "rails_helper"

RSpec.describe PushDeliveryJob, type: :job do
  # The notification's own after_create_commit broadcasts over ActionCable; stub
  # so the job spec stays isolated.
  before do
    allow_any_instance_of(Notification).to receive(:broadcast_replace_to)
    allow_any_instance_of(Notification).to receive(:broadcast_append_to)
  end

  let(:user) { create(:user) }
  let(:notification) do
    create(:notification, user: user, category: :document, priority: :awaiting,
           title: "Doc ready", body: "Invoice", link_url: "/documents/1")
  end

  let(:apns) { instance_double(Push::ApnsSender, deliver: :ok, close: nil) }
  let(:fcm)  { instance_double(Push::FcmSender, deliver: :ok) }

  before do
    allow(Push).to receive(:apns_configured?).and_return(true)
    allow(Push).to receive(:fcm_configured?).and_return(true)
    allow(Push::ApnsSender).to receive(:new).and_return(apns)
    allow(Push::FcmSender).to receive(:new).and_return(fcm)
  end

  it "delivers to each platform's devices via the matching sender" do
    ios = create(:device, user: user, platform: :ios, token: "ios1")
    android = create(:device, user: user, platform: :android, token: "and1")

    described_class.perform_now(notification.id)

    expect(apns).to have_received(:deliver).with(ios, hash_including(title: "Doc ready", url: "/documents/1"))
    expect(fcm).to have_received(:deliver).with(android, hash_including(title: "Doc ready"))
  end

  it "prunes a device the provider reports as invalid" do
    create(:device, user: user, platform: :ios, token: "dead")
    allow(apns).to receive(:deliver).and_return(:invalid)

    expect {
      described_class.perform_now(notification.id)
    }.to change(user.devices, :count).by(-1)
  end

  it "keeps a device on a transient error" do
    create(:device, user: user, platform: :ios, token: "keep")
    allow(apns).to receive(:deliver).and_return(:error)

    expect {
      described_class.perform_now(notification.id)
    }.not_to change(user.devices, :count)
  end

  it "skips a platform whose provider is not configured" do
    allow(Push).to receive(:apns_configured?).and_return(false)
    create(:device, user: user, platform: :ios, token: "ios-x")

    described_class.perform_now(notification.id)

    expect(Push::ApnsSender).not_to have_received(:new)
  end

  it "isolates a failing send so other devices still receive" do
    bad = create(:device, user: user, platform: :ios, token: "bad")
    good = create(:device, user: user, platform: :android, token: "good")
    allow(apns).to receive(:deliver).and_raise(StandardError, "boom")

    expect {
      described_class.perform_now(notification.id)
    }.not_to raise_error
    expect(fcm).to have_received(:deliver).with(good, anything)
    expect(Device.exists?(bad.id)).to be(true) # not pruned on error
  end

  it "no-ops when the user has no devices" do
    expect { described_class.perform_now(notification.id) }.not_to raise_error
  end
end
