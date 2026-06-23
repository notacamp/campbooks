# frozen_string_literal: true

class SecurityMethodCardComponentPreview < ViewComponent::Preview
  SHIELD = "M9 12.75 11.25 15 15 9.75m-3-7.036A11.959 11.959 0 0 1 3.598 6 11.99 11.99 0 0 0 3 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285Z"

  # @label On (with icon)
  def enabled
    render(Campbooks::SecurityMethodCard.new(
      title: "Authenticator app", enabled: true, icon: SHIELD,
      description: "Generate a 6-digit code with an app like 1Password or Authy."
    ))
  end

  # @label Off (no icon)
  def disabled
    render(Campbooks::SecurityMethodCard.new(
      title: "Email code", enabled: false,
      description: "Receive a one-time code by email at sign-in."
    ))
  end

  # @label With action body
  def with_body
    render(Campbooks::SecurityMethodCard.new(
      title: "Passkeys & security keys", enabled: false, icon: SHIELD,
      description: "Use Touch ID, Face ID, Windows Hello, or a hardware key."
    ) { "Action buttons / details render here." })
  end
end
