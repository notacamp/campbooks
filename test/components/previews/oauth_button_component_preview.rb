# frozen_string_literal: true

class OauthButtonComponentPreview < ViewComponent::Preview
  # Zoho OAuth button (md size)
  def zoho
    render Campbooks::OauthButton.new(provider: :zoho, href: "#", size: :md)
  end

  # Google OAuth button (md size)
  def google
    render Campbooks::OauthButton.new(provider: :google, href: "#", size: :md)
  end

  # Microsoft 365 OAuth button (md size)
  def microsoft
    render Campbooks::OauthButton.new(provider: :microsoft, href: "#", size: :md)
  end

  # Large Zoho button (full-width)
  def zoho_large
    render Campbooks::OauthButton.new(provider: :zoho, href: "#", size: :lg)
  end

  # Large Google button (full-width)
  def google_large
    render Campbooks::OauthButton.new(provider: :google, href: "#", size: :lg)
  end

  # Large Microsoft button (full-width)
  def microsoft_large
    render Campbooks::OauthButton.new(provider: :microsoft, href: "#", size: :lg)
  end

  # All providers at md size side by side
  def md_gallery
    html = [
      render(Campbooks::OauthButton.new(provider: :zoho, href: "#", size: :md)),
      render(Campbooks::OauthButton.new(provider: :google, href: "#", size: :md)),
      render(Campbooks::OauthButton.new(provider: :microsoft, href: "#", size: :md))
    ].join
    "<div class=\"flex flex-wrap items-center gap-4 p-6\">#{html}</div>".html_safe
  end

  # All providers at lg size stacked
  def lg_gallery
    html = [
      render(Campbooks::OauthButton.new(provider: :zoho, href: "#", size: :lg)),
      render(Campbooks::OauthButton.new(provider: :google, href: "#", size: :lg)),
      render(Campbooks::OauthButton.new(provider: :microsoft, href: "#", size: :lg))
    ].join
    "<div class=\"max-w-md space-y-3 p-6\">#{html}</div>".html_safe
  end
end
