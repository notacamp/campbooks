# Preview for Campbooks::StarToggle — the star / unstar toggle used on document rows
# and cards. Shows both states at both sizes.
class StarToggleComponentPreview < ViewComponent::Preview
  def unstarred
    render Campbooks::StarToggle.new(url: "#", starred: false)
  end

  def starred
    render Campbooks::StarToggle.new(url: "#", starred: true)
  end

  # @label Medium · unstarred
  def medium_unstarred
    render Campbooks::StarToggle.new(url: "#", starred: false, size: :md)
  end

  # @label Medium · starred
  def medium_starred
    render Campbooks::StarToggle.new(url: "#", starred: true, size: :md)
  end
end
