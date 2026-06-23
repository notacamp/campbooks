# frozen_string_literal: true

class BrandLogoComponentPreview < ViewComponent::Preview
  def google_drive
    render(Campbooks::BrandLogo.new(brand: :google_drive, size: :lg))
  end

  def notion
    render(Campbooks::BrandLogo.new(brand: :notion, size: :lg))
  end

  def zoho
    render(Campbooks::BrandLogo.new(brand: :zoho, size: :lg))
  end

  def fallback
    render(Campbooks::BrandLogo.new(brand: :unknown, size: :lg))
  end

  # @param brand select { choices: [google_drive, notion, zoho, unknown] }
  # @param size select { choices: [xs, sm, md, lg] }
  def playground(brand: :notion, size: :md)
    render(Campbooks::BrandLogo.new(brand: brand.to_sym, size: size.to_sym))
  end
end
