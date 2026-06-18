//! List available models with optional fuzzy search


/// Format a number as human-readable (e.g., 200000 -> "200K", 1000000 -> "1M")
fn format_token_count(count: u64) -> String {
    if count >= 1_000_000 {
        let millions = count as f64 / 1_000_000.0;
        if millions.fract() == 0.0 {
            format!("{}M", millions as u64)
        } else {
            format!("{:.1}M", millions)
        }
    } else if count >= 1_000 {
        let thousands = count as f64 / 1_000.0;
        if thousands.fract() == 0.0 {
            format!("{}K", thousands as u64)
        } else {
            format!("{:.1}K", thousands)
        }
    } else {
        count.to_string()
    }
}

/// List available models, optionally filtered by search pattern
pub async fn list_models(
    model_registry: &crate::core::model_registry::ModelRegistry,
    search_pattern: Option<&str>,
) {
    let models = model_registry.get_available();

    if models.is_empty() {
        println!("No models available. Configure a provider with `Pick config`.");
        return;
    }

    // Filter by search pattern if provided (simple substring match)
    let filtered: Vec<_> = if let Some(pattern) = search_pattern {
        let lower = pattern.to_lowercase();
        models.iter()
            .filter(|m| m.id.to_lowercase().contains(&lower) || m.provider.to_lowercase().contains(&lower))
            .collect()
    } else {
        models.iter().collect()
    };

    if filtered.is_empty() {
        if let Some(pattern) = search_pattern {
            println!("No models matching \"{}\"", pattern);
        }
        return;
    }

    // Sort by provider, then by model id
    let mut sorted = filtered.clone();
    sorted.sort_by(|a, b| {
        a.provider.cmp(&b.provider)
            .then_with(|| a.id.cmp(&b.id))
    });

    // Build rows
    struct Row {
        provider: String,
        model: String,
        context: String,
        max_out: String,
        thinking: String,
        images: String,
    }

    let rows: Vec<Row> = sorted.iter().map(|m| {
        let context = format_token_count(m.context_window);
        let max_out = format_token_count(m.max_tokens);
        let thinking = if m.reasoning { "yes" } else { "no" };
        let images = if m.input.iter().any(|i| i.contains("image")) { "yes" } else { "no" };
        Row {
            provider: m.provider.clone(),
            model: m.id.clone(),
            context,
            max_out,
            thinking: thinking.to_string(),
            images: images.to_string(),
        }
    }).collect();

    // Calculate column widths
    let provider_w = std::cmp::max(8, rows.iter().map(|r| r.provider.len()).max().unwrap_or(0));
    let model_w = std::cmp::max(5, rows.iter().map(|r| r.model.len()).max().unwrap_or(0));
    let context_w = std::cmp::max(7, rows.iter().map(|r| r.context.len()).max().unwrap_or(0));
    let max_out_w = std::cmp::max(7, rows.iter().map(|r| r.max_out.len()).max().unwrap_or(0));
    let thinking_w = 8;
    let images_w = 6;

    // Print header
    println!(
        "{:<pw$}  {:<mw$}  {:<cw$}  {:<ow$}  {:<tw$}  {:<iw$}",
        "provider", "model", "context", "max-out", "thinking", "images",
        pw = provider_w, mw = model_w, cw = context_w, ow = max_out_w, tw = thinking_w, iw = images_w
    );

    // Print rows
    for row in &rows {
        println!(
            "{:<pw$}  {:<mw$}  {:<cw$}  {:<ow$}  {:<tw$}  {:<iw$}",
            row.provider, row.model, row.context, row.max_out, row.thinking, row.images,
            pw = provider_w, mw = model_w, cw = context_w, ow = max_out_w, tw = thinking_w, iw = images_w
        );
    }
}
