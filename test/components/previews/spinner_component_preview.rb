# frozen_string_literal: true

class SpinnerComponentPreview < ViewComponent::Preview
  def sm
    render(Campbooks::Spinner.new(size: :sm))
  end

  def md
    render(Campbooks::Spinner.new)
  end

  def lg
    render(Campbooks::Spinner.new(size: :lg))
  end

  def sizes
    render(Campbooks::SpinnerSizes.new)
  end

  def with_text
    "<div class=\"flex items-center gap-2 p-6\">#{Campbooks::Spinner.new(size: :sm).call} Processing...</div>".html_safe
  end
end
