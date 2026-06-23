require "rails_helper"

RSpec.describe "Devices", type: :request do
  let(:user) { create(:user) }

  describe "POST /device" do
    it "rejects unauthenticated registration" do
      post device_path, params: { platform: "ios", token: "tok" }

      expect(Device.count).to eq(0)
      expect(response).to redirect_to("/session/new")
    end

    context "when signed in" do
      before { sign_in(user) }

      it "registers a device token for the current user" do
        expect {
          post device_path, params: { platform: "ios", token: "apns-tok", app_version: "1.2.3" }
        }.to change(user.devices, :count).by(1)

        expect(response).to have_http_status(:created)
        device = user.devices.last
        expect(device).to be_ios
        expect(device.token).to eq("apns-tok")
      end

      it "is idempotent for the same token" do
        post device_path, params: { platform: "android", token: "fcm-tok" }

        expect {
          post device_path, params: { platform: "android", token: "fcm-tok" }
        }.not_to change(Device, :count)
      end

      it "rejects an unknown platform" do
        post device_path, params: { platform: "windows", token: "x" }

        expect(response).to have_http_status(:unprocessable_entity)
      end
    end
  end

  describe "DELETE /device" do
    before { sign_in(user) }

    it "removes the caller's device by token" do
      create(:device, user: user, token: "gone")

      expect {
        delete device_path, params: { token: "gone" }
      }.to change(user.devices, :count).by(-1)

      expect(response).to have_http_status(:no_content)
    end

    it "does not remove another user's device" do
      other = create(:user)
      create(:device, user: other, token: "theirs")

      delete device_path, params: { token: "theirs" }

      expect(Device.find_by(token: "theirs")).to be_present
    end
  end
end
