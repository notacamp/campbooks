class AiSetupMessageComponentPreview < ViewComponent::Preview
  def scout_question
    message = AgentMessage.new(content: "What kinds of documents does your business handle most?", author_type: :ai)
    render Campbooks::AiSetupMessage.new(message: message, hint: "invoices, contracts, receipts")
  end

  def scout_question_without_hint
    message = AgentMessage.new(content: "Great — anything else you'd like Scout to keep an eye out for?", author_type: :ai)
    render Campbooks::AiSetupMessage.new(message: message)
  end

  def user_answer
    message = AgentMessage.new(content: "We're a small law firm — mostly contracts and client invoices.", author_type: :user)
    render Campbooks::AiSetupMessage.new(message: message)
  end
end
