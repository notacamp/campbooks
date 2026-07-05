require "apnotic"

module Push
  # Sends one push to one iOS device via APNs (HTTP/2, token-based .p8 auth).
  #
  # #deliver returns one of:
  #   :ok      — accepted by APNs
  #   :invalid — the token is dead (unregistered / wrong topic); caller prunes it
  #   :error   — transient/other failure; left in place to try again next time
  #
  # The connection is injectable so specs can drive #deliver without a real key.
  class ApnsSender
    def initialize(connection: nil)
      @connection = connection || build_connection
    end

    def deliver(device, title:, body: nil, url: nil, badge: nil, category: nil)
      notification = Apnotic::Notification.new(device.token)
      notification.topic = Push.apns_bundle_id
      notification.alert = { title: title, body: body }.compact
      notification.sound = "default"
      notification.badge = badge if badge
      notification.custom_payload = { url: url, category: category }.compact

      start = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      response = @connection.push(notification)
      result = classify(response, device)
      duration_ms = ((Process.clock_gettime(Process::CLOCK_MONOTONIC) - start) * 1000).round
      record_apns(result, response, duration_ms)
      result
    end

    def close
      @connection&.close
    end

    private

    def classify(response, device)
      return log_and(:error, "timeout", device) if response.nil?

      case response.status.to_i
      when 200 then :ok
      when 410 then :invalid # device no longer registered
      when 400
        reason = response.body.is_a?(Hash) ? response.body["reason"] : nil
        %w[BadDeviceToken DeviceTokenNotForTopic Unregistered].include?(reason) ? :invalid : log_and(:error, reason, device)
      else
        log_and(:error, "#{response.status} #{response.body}", device)
      end
    end

    def record_apns(result, response, duration_ms)
      case result
      when :ok
        SystemHealth.record(service: "push_apns", operation: "deliver", status: :success, duration_ms: duration_ms)
      when :invalid
        SystemHealth.record(service: "push_apns", operation: "deliver", status: :success, duration_ms: duration_ms, http_status: 410, metadata: { result: "invalid_token" })
      else
        reason = response ? "#{response.status} #{response.body}" : "timeout"
        SystemHealth.record(service: "push_apns", operation: "deliver", status: :error, duration_ms: duration_ms, error_message: SystemHealth.sanitize_message(reason))
      end
    end

    def log_and(result, detail, device)
      Rails.logger.error("[push] APNs #{detail} for device ##{device.id}")
      result
    end

    def build_connection
      options = {
        auth_method: :token,
        cert_path: Push.apns_key_path,
        key_id: Push.apns_key_id,
        team_id: Push.apns_team_id
      }
      if Push.apns_environment.to_s == "production"
        Apnotic::Connection.new(options)
      else
        Apnotic::Connection.development(options)
      end
    end
  end
end
