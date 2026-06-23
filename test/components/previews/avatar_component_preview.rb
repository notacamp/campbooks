# frozen_string_literal: true

class AvatarComponentPreview < ViewComponent::Preview
  def sm
    render(Campbooks::Avatar.new(name: "Alice Smith", size: :sm))
  end

  def md
    render(Campbooks::Avatar.new(name: "Alice Smith", size: :md))
  end

  def lg
    render(Campbooks::Avatar.new(name: "Alice Smith", size: :lg))
  end

  def sizes
    render(Campbooks::AvatarSizes.new(name: "Alice Smith"))
  end

  def single_initial
    render(Campbooks::Avatar.new(name: "Alice", size: :md))
  end

  def no_name
    render(Campbooks::Avatar.new(size: :md))
  end

  def no_name_sizes
    render(Campbooks::AvatarSizes.new)
  end
end
