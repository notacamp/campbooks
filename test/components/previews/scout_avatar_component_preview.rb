# @label Scout Avatar
class ScoutAvatarComponentPreview < Lookbook::Preview
  # @param size select [xs, sm, md, lg, xl]
  # @param pulse toggle
  def default(size: :lg, pulse: false)
    render Campbooks::ScoutAvatar.new(size: size.to_sym, pulse: pulse)
  end

  def xl
    render Campbooks::ScoutAvatar.new(size: :xl)
  end

  # @label Thinking (pulse)
  def pulse
    render Campbooks::ScoutAvatar.new(size: :lg, pulse: true)
  end
end
