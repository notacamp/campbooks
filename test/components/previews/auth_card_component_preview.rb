# frozen_string_literal: true

class AuthCardComponentPreview < ViewComponent::Preview
  # The framed shell behind every unauthenticated screen (sign in, sign up,
  # password reset): a raised paper card centered on a warm-grey canvas lit by
  # one faint Ember halo. Each template wraps the card in the same canvas the
  # real <main> supplies via `auth_canvas_class`. Toggle Lookbook's theme to see
  # the halo glow on dark.

  # @label Sign in (full)
  def sign_in
    render_with_template(template: "auth_card_component_preview/sign_in")
  end

  # @label Minimal (title + one field)
  def minimal
    render_with_template(template: "auth_card_component_preview/minimal")
  end

  # @label Without the Beta badge (self-hosted)
  def without_badge
    render_with_template(template: "auth_card_component_preview/without_badge")
  end
end
