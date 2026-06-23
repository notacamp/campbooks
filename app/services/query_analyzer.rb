class QueryAnalyzer
  attr_reader :matched_tags, :intents, :temporal_hint

  def initialize(query, workspace:)
    @query = query.to_s.downcase
    @workspace = workspace
    @matched_tags = []
    @intents = []
    @temporal_hint = nil
  end

  def analyze
    detect_tags
    detect_temporal_hint
    detect_type_intent
    self
  end

  def tag_boost_weight
    0.3
  end

  private

  def detect_tags
    tags = @workspace.tags.active.by_name
    return if tags.empty?

    tags.each do |tag|
      confidence = match_confidence(tag)
      if confidence > 0
        @matched_tags << { tag: tag, confidence: confidence }
      end
    end

    @matched_tags.sort_by! { |m| -m[:confidence] }
  end

  def match_confidence(tag)
    name = tag.name.downcase

    # Exact name match
    return 0.95 if @query.include?(name)

    # Word boundary match (tag name as whole word in query)
    return 0.80 if @query.match?(/\b#{Regexp.escape(name)}\b/i)

    # Prompt keyword match (if tag has a prompt with relevant words)
    prompt_text = tag.prompt.to_s.downcase
    if prompt_text.present?
      keywords = prompt_text.split(/\s+/).reject { |w| w.length < 4 || common_words.include?(w) }
      matching_keywords = keywords.count { |kw| @query.include?(kw) }
      if matching_keywords > 0
        return [ 0.5 + (matching_keywords.to_f / keywords.length) * 0.4, 0.85 ].min
      end
    end

    0
  end

  def detect_temporal_hint
    case @query
    when /\b(today|hoje)\b/i
      @temporal_hint = { since: Date.current.beginning_of_day }
    when /\b(yesterday|ontem)\b/i
      @temporal_hint = { since: 1.day.ago.beginning_of_day, until: 1.day.ago.end_of_day }
    when /\bthis week|esta semana\b/i
      @temporal_hint = { since: Date.current.beginning_of_week }
    when /\blast week|semana passada\b/i
      @temporal_hint = { since: 1.week.ago.beginning_of_week, until: 1.week.ago.end_of_week }
    when /\bthis month|este m[eê]s\b/i
      @temporal_hint = { since: Date.current.beginning_of_month }
    when /\blast month|m[eê]s passado\b/i
      @temporal_hint = { since: 1.month.ago.beginning_of_month, until: 1.month.ago.end_of_month }
    when /\b(recent|recently|latest|newest|recentemente|último)\b/i
      @temporal_hint = { boost_recent: true }
    end
  end

  def detect_type_intent
    @intents << :emails if @query.match?(/\b(email|message|mail|thread|inbox)s?\b/i)
    @intents << :contacts if @query.match?(/\b(contact|person|people|who is|quem [eé])\b/i)
    @intents << :documents if @query.match?(/\b(document|invoice|receipt|contract|certificate|fatura|recibo|contrato)s?\b/i)
    @intents << :financial if @query.match?(/\b(amount|value|money|payment|valor|pagamento|montante)\b/i)
    @intents << :urgent if @query.match?(/\b(urgent|important|priority|high|urgente|importante|prioridade)\b/i)
  end

  def common_words
    %w[the and for that with from have this about what when where which who how all are not was has been can will would should may com que para com de uma um dos das the and for]
  end
end
