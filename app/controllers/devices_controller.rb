# Registers/removes a native device's push token for the signed-in user.
#
# Called by the Hotwire Native shell through the push bridge: the bridge hands
# the token to the web view's JS, which POSTs it here with the normal session
# cookie + CSRF token, so authentication and forgery protection are unchanged.
class DevicesController < ApplicationController
  def create
    Device.register!(
      user: Current.user,
      platform: device_params[:platform],
      token: device_params[:token],
      app_version: device_params[:app_version]
    )
    head :created
  rescue ArgumentError
    # Unknown platform value (enum guard).
    head :unprocessable_entity
  end

  def destroy
    Current.user.devices.find_by(token: params[:token])&.destroy
    head :no_content
  end

  private

  def device_params
    params.permit(:platform, :token, :app_version)
  end
end
